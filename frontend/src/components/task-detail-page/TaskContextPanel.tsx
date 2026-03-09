import { useState } from 'react';
import {
  getTaskContextAttachmentDownloadUrl,
  type TaskContext,
  type TaskContextAttachment,
} from '../../api/taskContexts';
import { logger } from '@/lib/logger';

interface TaskContextPanelProps {
  taskId: string;
  contexts: TaskContext[];
  loading?: boolean;
  error?: string | null;
}

function formatBytes(bytes?: number | null): string {
  if (!bytes) return 'Unknown size';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function attachmentMode(attachment: TaskContextAttachment): {
  label: string;
  tone: string;
} {
  const contentType = attachment.content_type;
  const size = attachment.size_bytes ?? Number.MAX_SAFE_INTEGER;

  if (contentType.startsWith('text/') || contentType === 'application/json' || contentType === 'text/yaml' || contentType === 'application/yaml' || contentType === 'application/x-yaml') {
    if (size <= 1_048_576) {
      return { label: 'Prompt text', tone: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30' };
    }
    return { label: 'Link only', tone: 'bg-amber-500/15 text-amber-300 border-amber-500/30' };
  }

  if (contentType === 'image/png' || contentType === 'image/jpeg' || contentType === 'image/webp') {
    return { label: 'Vision reference', tone: 'bg-sky-500/15 text-sky-300 border-sky-500/30' };
  }

  return { label: 'Link only', tone: 'bg-amber-500/15 text-amber-300 border-amber-500/30' };
}

export function TaskContextPanel({
  taskId,
  contexts,
  loading = false,
  error = null,
}: TaskContextPanelProps) {
  const [downloadingKey, setDownloadingKey] = useState<string | null>(null);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  const totalAttachments = contexts.reduce((sum, context) => sum + context.attachments.length, 0);
  const oversizedAttachments = contexts.flatMap((context) =>
    context.attachments.filter((attachment) => (attachment.size_bytes ?? 0) > 1_048_576)
  );
  const autoResolvedCount = contexts
    .flatMap((context) => context.attachments)
    .filter((attachment) => {
      const contentType = attachment.content_type;
      const size = attachment.size_bytes ?? Number.MAX_SAFE_INTEGER;
      return (
        size <= 1_048_576 &&
        (contentType.startsWith('text/') ||
          contentType === 'application/json' ||
          contentType === 'text/yaml' ||
          contentType === 'application/yaml' ||
          contentType === 'application/x-yaml')
      );
    }).length;

  const handleDownload = async (attachment: TaskContextAttachment) => {
    try {
      setDownloadingKey(attachment.storage_key);
      setDownloadError(null);
      const { download_url } = await getTaskContextAttachmentDownloadUrl(taskId, attachment.storage_key);
      window.open(download_url, '_blank', 'noopener,noreferrer');
    } catch (err) {
      logger.error('Failed to get task context attachment download URL', err);
      setDownloadError('Failed to open attachment');
    } finally {
      setDownloadingKey(null);
    }
  };

  return (
    <section className="bg-card border border-border rounded-xl p-6">
      <div className="flex items-start justify-between gap-4 mb-4">
        <div>
          <h3 className="text-xs font-bold text-card-foreground uppercase tracking-wider mb-1">
            Task Context
          </h3>
          <p className="text-sm text-muted-foreground">
            This is the task-specific context that will be injected before the agent starts.
          </p>
        </div>
        <div className="text-right">
          <div className="text-sm font-semibold text-card-foreground">{contexts.length} block{contexts.length === 1 ? '' : 's'}</div>
          <div className="text-xs text-muted-foreground">{totalAttachments} attachment{totalAttachments === 1 ? '' : 's'}</div>
        </div>
      </div>

      {loading ? (
        <div className="animate-pulse rounded-lg border border-border bg-muted/50 h-32" />
      ) : error ? (
        <div className="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-400">
          {error}
        </div>
      ) : contexts.length === 0 ? (
        <div className="rounded-lg border border-border bg-muted/40 px-4 py-4 text-sm text-muted-foreground">
          No task context attached yet.
        </div>
      ) : (
        <div className="space-y-4">
          {(autoResolvedCount > 5 || oversizedAttachments.length > 0) && (
            <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-200">
              {autoResolvedCount > 5 && (
                <div>Only the first 5 text attachments will be injected into the prompt.</div>
              )}
              {oversizedAttachments.length > 0 && (
                <div>Attachments larger than 1 MiB will be passed as links instead of inline text.</div>
              )}
            </div>
          )}

          {downloadError && (
            <div className="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-400">
              {downloadError}
            </div>
          )}

          {contexts.map((context, index) => (
            <div key={context.id} className="rounded-xl border border-border bg-muted/30 p-4 space-y-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="text-sm font-semibold text-card-foreground">
                    {context.title?.trim() || `Context ${index + 1}`}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    {context.content_type} • source: {context.source}
                  </div>
                </div>
                <div className="text-xs text-muted-foreground">
                  order {context.sort_order}
                </div>
              </div>

              {context.raw_content.trim() && (
                <div className="rounded-lg border border-border bg-card px-4 py-3 text-sm text-card-foreground whitespace-pre-wrap break-words">
                  {context.raw_content}
                </div>
              )}

              {context.attachments.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                    Attachments
                  </div>
                  {context.attachments.map((attachment) => {
                    const mode = attachmentMode(attachment);
                    const isDownloading = downloadingKey === attachment.storage_key;

                    return (
                      <div
                        key={attachment.id}
                        className="rounded-lg border border-border bg-card px-3 py-3 flex items-center justify-between gap-3"
                      >
                        <div className="min-w-0">
                          <div className="flex items-center gap-2 flex-wrap">
                            <p className="text-sm text-card-foreground truncate">{attachment.filename}</p>
                            <span className={`inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium ${mode.tone}`}>
                              {mode.label}
                            </span>
                          </div>
                          <p className="text-xs text-muted-foreground">
                            {attachment.content_type} • {formatBytes(attachment.size_bytes ?? undefined)}
                          </p>
                        </div>
                        <button
                          type="button"
                          onClick={() => void handleDownload(attachment)}
                          disabled={isDownloading}
                          className="px-3 py-2 bg-card border border-border hover:bg-muted text-card-foreground text-xs font-medium rounded-lg flex items-center gap-1.5 transition-all disabled:opacity-50"
                        >
                          {isDownloading ? (
                            <span className="w-3.5 h-3.5 border-2 border-muted-foreground/30 border-t-muted-foreground rounded-full animate-spin" />
                          ) : (
                            <span className="material-symbols-outlined text-[16px]">download</span>
                          )}
                          Open
                        </button>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
