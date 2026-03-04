import {
  useState,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  KeyboardEvent,
  ChangeEvent,
} from 'react';
import {
  Send,
  Paperclip,
  X,
  Loader2,
  RotateCcw,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  CircleDot,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { AutoExpandingTextarea } from '@/components/ui/auto-expanding-textarea';
import { useFollowUpSend } from '@/hooks/useFollowUpSend';
import { useExecutionProcessReset } from '@/hooks/useExecutionProcessReset';
import { getTaskAttachmentUploadUrl, getTask, updateTaskMetadata } from '@/api/tasks';
import type { TimelineTokenUsageInfo } from '@/types/timeline-log';
import { cn } from '@/lib/utils';
import { logger } from '@/lib/logger';

export interface TaskFollowUpSectionProps {
  sessionId: string;
  isRunning: boolean;
  disabled?: boolean;
  retryProcessId?: string | null;
  taskId?: string | null;
  projectId?: string | null;
  attemptStatus?: string | null;
  /** Error message when attempt failed (e.g. git clone failed) */
  attemptErrorMessage?: string | null;
  tokenUsageInfo?: TimelineTokenUsageInfo | null;
}

interface PendingAttachment {
  id: string;
  filename: string;
  contentType: string;
  size: number;
  key?: string;
  status: 'uploading' | 'uploaded' | 'failed';
  error?: string;
}

interface UploadedAttachmentMetadata {
  key: string;
  filename: string;
  content_type: string;
  size: number;
  uploaded_at: string;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function TaskFollowUpSection({
  sessionId,
  isRunning,
  disabled = false,
  retryProcessId = null,
  taskId = null,
  projectId = null,
  attemptStatus = null,
  attemptErrorMessage = null,
  tokenUsageInfo = null,
}: TaskFollowUpSectionProps) {
  const [message, setMessage] = useState('');
  const [attachments, setAttachments] = useState<PendingAttachment[]>([]);
  const attachmentInputRef = useRef<HTMLInputElement | null>(null);

  const handleCleanup = useCallback(() => {
    setMessage('');
    setAttachments([]);
  }, []);

  const {
    isSendingFollowUp,
    followUpError,
    setFollowUpError,
    onSendFollowUp,
  } = useFollowUpSend({
    sessionId,
    isRunning,
    message,
    retryProcessId,
    onAfterSendCleanup: handleCleanup,
  });

  const {
    isResetting,
    resetError,
    resetInfo,
    requiresForceReset,
    resetProcess,
    clearResetState,
  } = useExecutionProcessReset();

  useEffect(() => {
    clearResetState();
  }, [retryProcessId, clearResetState]);

  const isDisabled = disabled || isSendingFollowUp;
  const isFollowUpContextReady = isRunning || Boolean(retryProcessId);
  const canReset = !isDisabled && !isRunning && Boolean(retryProcessId);

  const setAttachmentState = useCallback(
    (id: string, updater: (item: PendingAttachment) => PendingAttachment) => {
      setAttachments((prev) => prev.map((item) => (item.id === id ? updater(item) : item)));
    },
    []
  );

  const uploadedAttachments = useMemo<UploadedAttachmentMetadata[]>(
    () =>
      attachments
        .filter(
          (attachment): attachment is PendingAttachment & { key: string } =>
            attachment.status === 'uploaded' && Boolean(attachment.key)
        )
        .map((attachment) => ({
          key: attachment.key,
          filename: attachment.filename,
          content_type: attachment.contentType,
          size: attachment.size,
          uploaded_at: new Date().toISOString(),
        })),
    [attachments]
  );

  const hasUploadingAttachments = attachments.some((attachment) => attachment.status === 'uploading');
  const hasAttachmentFailures = attachments.some((attachment) => attachment.status === 'failed');

  const uploadFiles = useCallback(
    async (files: File[]) => {
      if (!projectId) {
        setFollowUpError('Project context is missing. Cannot upload references right now.');
        return;
      }

      for (const file of files) {
        const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
        const contentType = file.type || 'application/octet-stream';

        setAttachments((prev) => [
          ...prev,
          {
            id,
            filename: file.name,
            contentType,
            size: file.size,
            status: 'uploading',
          },
        ]);

        try {
          const { upload_url, key } = await getTaskAttachmentUploadUrl({
            project_id: projectId,
            filename: file.name,
            content_type: contentType,
          });

          const response = await fetch(upload_url, {
            method: 'PUT',
            headers: {
              'Content-Type': contentType,
            },
            body: file,
          });

          if (!response.ok) {
            throw new Error(`Upload failed with status ${response.status}`);
          }

          setAttachmentState(id, (item) => ({
            ...item,
            key,
            status: 'uploaded',
            error: undefined,
          }));
        } catch (error) {
          logger.error('Follow-up attachment upload failed:', error);
          setAttachmentState(id, (item) => ({
            ...item,
            status: 'failed',
            error: 'Upload failed',
          }));
        }
      }
    },
    [projectId, setAttachmentState, setFollowUpError]
  );

  const persistUploadedAttachmentsToTask = useCallback(async () => {
    if (uploadedAttachments.length === 0) return;

    if (!taskId) {
      throw new Error('Task context is missing. Cannot attach reference files for follow-up.');
    }

    const task = await getTask(taskId);
    const metadata =
      task.metadata && typeof task.metadata === 'object' && !Array.isArray(task.metadata)
        ? task.metadata
        : {};
    const existingAttachments = Array.isArray((metadata as Record<string, unknown>).attachments)
      ? ((metadata as Record<string, unknown>).attachments as Array<Record<string, unknown>>)
      : [];

    const deduped = new Map<string, Record<string, unknown>>();
    existingAttachments.forEach((attachment) => {
      const key = typeof attachment?.key === 'string' ? attachment.key : '';
      if (key) deduped.set(key, attachment);
    });
    uploadedAttachments.forEach((attachment) => {
      deduped.set(attachment.key, attachment as unknown as Record<string, unknown>);
    });

    const mergedAttachments = Array.from(deduped.values());
    const nextMetadata = {
      ...metadata,
      attachments: mergedAttachments,
      attachments_count: mergedAttachments.length,
    };

    await updateTaskMetadata(taskId, nextMetadata);
  }, [taskId, uploadedAttachments]);

  const handleSend = useCallback(async () => {
    if (disabled || isSendingFollowUp) return;

    if (!message.trim()) {
      setFollowUpError('Please enter follow-up instructions for the agent.');
      return;
    }

    if (hasUploadingAttachments) {
      setFollowUpError('Please wait until reference files finish uploading.');
      return;
    }

    try {
      await persistUploadedAttachmentsToTask();
    } catch (error) {
      const errorMessage =
        error instanceof Error
          ? error.message
          : 'Failed to save reference files for follow-up.';
      setFollowUpError(errorMessage);
      return;
    }

    await onSendFollowUp();
  }, [
    disabled,
    isSendingFollowUp,
    message,
    hasUploadingAttachments,
    persistUploadedAttachmentsToTask,
    setFollowUpError,
    onSendFollowUp,
  ]);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
      e.preventDefault();
      void handleSend();
    }
  };

  const handleAttachmentInputChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files ? Array.from(event.target.files) : [];
    event.target.value = '';
    if (files.length === 0) return;
    await uploadFiles(files);
  };

  const handleRemoveAttachment = useCallback((attachmentId: string) => {
    setAttachments((prev) => prev.filter((item) => item.id !== attachmentId));
  }, []);

  const canAttach = !isDisabled && !isRunning && Boolean(projectId) && Boolean(taskId);
  const attachTooltip = !projectId || !taskId
    ? 'Reference upload requires task/project context'
    : isRunning
      ? 'Reference upload is available when the attempt is stopped'
      : 'Attach reference files (images, videos, docs)';
  const canSend =
    !isDisabled &&
    isFollowUpContextReady &&
    message.trim().length > 0 &&
    !hasUploadingAttachments;
  const normalizedAttemptStatus = attemptStatus?.toLowerCase() ?? null;

  const handleResetProcess = () => {
    void resetProcess(retryProcessId);
  };

  const handleOpenAttachmentPicker = () => {
    if (!canAttach) return;
    attachmentInputRef.current?.click();
  };

  return (
    <div className="border-t border-border bg-background">
      {/* Attempt failure error (e.g. git clone failed) */}
      {attemptStatus === 'failed' && attemptErrorMessage && (
        <div className="px-4 py-2 bg-destructive/10 border-b border-destructive/30">
          <div className="flex items-start gap-2">
            <AlertTriangle className="h-4 w-4 text-destructive mt-0.5 shrink-0" />
            <span className="text-sm text-destructive">{attemptErrorMessage}</span>
          </div>
        </div>
      )}
      {/* Error message */}
      {followUpError && (
        <div className="px-4 py-2 bg-destructive/10 border-b border-destructive/30">
          <div className="flex items-start gap-2">
            <span className="text-sm text-destructive">{followUpError}</span>
            <button
              onClick={() => setFollowUpError(null)}
              className="ml-auto text-destructive hover:text-destructive/80"
              aria-label="Dismiss error"
            >
              ×
            </button>
          </div>
        </div>
      )}

      {resetError && (
        <div
          className={`px-4 py-2 border-b ${
            requiresForceReset
              ? 'bg-amber-500/10 border-amber-500/30'
              : 'bg-destructive/10 border-destructive/30'
          }`}
        >
          <div className="flex items-start gap-2">
            {requiresForceReset ? (
              <AlertTriangle className="h-4 w-4 text-amber-500 mt-0.5 shrink-0" />
            ) : (
              <AlertTriangle className="h-4 w-4 text-destructive mt-0.5 shrink-0" />
            )}
            <span
              className={`text-sm ${
                requiresForceReset ? 'text-amber-500' : 'text-destructive'
              }`}
            >
              {resetError}
            </span>
          </div>
        </div>
      )}

      {resetInfo && (
        <div className="px-4 py-2 bg-emerald-500/10 border-b border-emerald-500/30">
          <span className="text-sm text-emerald-500">{resetInfo}</span>
        </div>
      )}

      {/* Input area */}
      <div className="p-3">
        <div className="flex flex-col gap-2">
          <AutoExpandingTextarea
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={isRunning ? "Queue a follow-up (agent will process when current task completes)..." : "Send follow-up instructions to resume this attempt..."}
            disabled={isDisabled}
            minHeight={40}
            maxHeight={200}
            className="resize-none"
          />

          {attachments.length > 0 && (
            <div className="space-y-1">
              {attachments.map((attachment) => (
                <div
                  key={attachment.id}
                  className="rounded border border-border/70 bg-muted/30 px-2 py-1 flex items-center justify-between gap-2"
                >
                  <div className="min-w-0">
                    <p className="text-xs text-foreground truncate">{attachment.filename}</p>
                    <p className="text-[11px] text-muted-foreground">
                      {formatBytes(attachment.size)}
                      {attachment.status === 'uploading' && ' • uploading'}
                      {attachment.status === 'uploaded' && ' • uploaded'}
                      {attachment.status === 'failed' && ` • ${attachment.error ?? 'failed'}`}
                    </p>
                  </div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={() => handleRemoveAttachment(attachment.id)}
                    disabled={attachment.status === 'uploading'}
                    aria-label={`Remove ${attachment.filename}`}
                    title="Remove reference"
                  >
                    <X className="h-3.5 w-3.5" />
                  </Button>
                </div>
              ))}
            </div>
          )}

          {/* Action row */}
          <div className="flex items-center gap-3">
            {(normalizedAttemptStatus || tokenUsageInfo) && (
              <div className="flex items-center gap-2 min-w-0">
                <div className="flex items-center gap-1.5 min-w-0">
                  {normalizedAttemptStatus === 'failed' ? (
                    <XCircle className="h-3.5 w-3.5 text-destructive shrink-0" />
                  ) : normalizedAttemptStatus === 'cancelled' ? (
                    <CircleDot className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                  ) : (
                    <CheckCircle2 className="h-3.5 w-3.5 text-success shrink-0" />
                  )}
                  <span
                    className={`text-xs font-medium ${
                      normalizedAttemptStatus === 'failed'
                        ? 'text-destructive'
                        : normalizedAttemptStatus === 'cancelled'
                          ? 'text-muted-foreground'
                          : 'text-success'
                    }`}
                  >
                    {normalizedAttemptStatus === 'failed'
                      ? 'Failed'
                      : normalizedAttemptStatus === 'cancelled'
                        ? 'Cancelled'
                        : 'Completed'}
                  </span>
                </div>

                {tokenUsageInfo && <InlineContextUsageGauge tokenUsageInfo={tokenUsageInfo} />}
              </div>
            )}

            <div className="flex items-center gap-2 ml-auto">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleOpenAttachmentPicker}
                disabled={!canAttach}
                className="h-8 w-8 p-0 text-muted-foreground hover:text-foreground"
                title={attachTooltip}
                aria-label="Attach reference files"
              >
                <Paperclip className="h-4 w-4" />
              </Button>
              <input
                ref={attachmentInputRef}
                type="file"
                multiple
                className="hidden"
                accept="image/*,video/*,.pdf,.doc,.docx,.ppt,.pptx,.xls,.xlsx,.txt,.md,.json,.csv"
                onChange={handleAttachmentInputChange}
              />

              {/* Reset process worktree button */}
              <Button
                onClick={handleResetProcess}
                variant={requiresForceReset ? 'destructive' : 'outline'}
                size="sm"
                disabled={!canReset || isResetting}
                className="h-8 px-2 gap-1.5"
                title={
                  requiresForceReset
                    ? 'Force hard reset and discard uncommitted changes'
                    : 'Reset process worktree to HEAD'
                }
              >
                {isResetting ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : requiresForceReset ? (
                  <AlertTriangle className="h-4 w-4" />
                ) : (
                  <RotateCcw className="h-4 w-4" />
                )}
                <span className="text-xs">
                  {isResetting ? 'Resetting...' : requiresForceReset ? 'Force Reset' : 'Reset'}
                </span>
              </Button>

              {/* Send button */}
              <Button
                onClick={() => void handleSend()}
                disabled={!canSend}
                size="sm"
                className="h-8 gap-1.5"
              >
                {isSendingFollowUp ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    <span>Sending...</span>
                  </>
                ) : (
                  <>
                    <Send className="h-4 w-4" />
                    <span>Send</span>
                  </>
                )}
              </Button>
            </div>
          </div>
        </div>

        {/* Keyboard hint */}
        <div className="mt-2 text-xs text-muted-foreground">
          {isFollowUpContextReady
            ? hasAttachmentFailures
              ? 'Some references failed to upload. Remove failed items or retry upload before sending.'
              : 'Send Shift + Enter to send'
            : 'Waiting for execution process context to resume this attempt...'}
        </div>
      </div>
    </div>
  );
}

function InlineContextUsageGauge({ tokenUsageInfo }: { tokenUsageInfo: TimelineTokenUsageInfo }) {
  const hasContextWindow =
    typeof tokenUsageInfo.modelContextWindow === 'number' &&
    tokenUsageInfo.modelContextWindow > 0;
  const percentage = hasContextWindow
    ? Math.min(100, (tokenUsageInfo.totalTokens / tokenUsageInfo.modelContextWindow!) * 100)
    : 0;
  const progress = Math.min(1, Math.max(0, percentage / 100));

  let colorClass = 'text-emerald-500';
  if (hasContextWindow) {
    if (percentage >= 90) colorClass = 'text-destructive';
    else if (percentage >= 75) colorClass = 'text-orange-500';
    else if (percentage >= 50) colorClass = 'text-amber-500';
  } else {
    colorClass = 'text-sky-500';
  }

  const radius = 8;
  const strokeWidth = 2;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference * (1 - progress);

  const tooltip = hasContextWindow
    ? `Context usage ${Math.round(percentage)}%`
    : 'Token usage';

  return (
    <div
      className="inline-flex items-center justify-center h-7 w-7 rounded-sm text-muted-foreground hover:text-foreground transition-colors cursor-help"
      title={tooltip}
      aria-label={tooltip}
    >
      <svg viewBox="0 0 20 20" className="w-4 h-4 -rotate-90" aria-hidden="true">
        <circle
          cx="10"
          cy="10"
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={strokeWidth}
          className="text-border"
        />
        <circle
          cx="10"
          cy="10"
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={strokeWidth}
          strokeLinecap="round"
          strokeDasharray={`${circumference} ${circumference}`}
          strokeDashoffset={dashOffset}
          className={cn(colorClass, 'transition-all duration-500 ease-out')}
        />
      </svg>
    </div>
  );
}
