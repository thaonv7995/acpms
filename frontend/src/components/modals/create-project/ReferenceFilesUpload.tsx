/**
 * ReferenceFilesUpload - Upload reference files for project init (From Scratch only)
 *
 * Allows user to upload ZIP, images, PDF, etc. before creating project.
 * Agent will read these files from .acpms-refs/ to scaffold project better.
 */

import { useCallback } from 'react';
import { getInitRefUploadUrl } from '../../../api/projects';

const MAX_FILES = 5;
const MAX_SIZE_MB = 10;
const ALLOWED_TYPES = [
  'application/zip',
  'application/x-tar',
  'application/gzip',
  'image/png',
  'image/jpeg',
  'image/webp',
  'application/pdf',
  'text/plain',
  'text/markdown',
  'application/json',
];

interface RefAttachment {
  id: string;
  filename: string;
  contentType: string;
  size: number;
  status: 'uploading' | 'uploaded' | 'failed';
  key?: string;
  error?: string;
}

interface ReferenceFilesUploadProps {
  attachments: RefAttachment[];
  onAttachmentsChange: (updater: RefAttachment[] | ((prev: RefAttachment[]) => RefAttachment[])) => void;
}

export function ReferenceFilesUpload({
  attachments,
  onAttachmentsChange,
}: ReferenceFilesUploadProps) {
  const removeAttachment = useCallback(
    (id: string) => {
      onAttachmentsChange((prev) => prev.filter((item) => item.id !== id));
    },
    [onAttachmentsChange]
  );

  const hasUploading = attachments.some((a) => a.status === 'uploading');
  const canAddMore = attachments.length < MAX_FILES;

  const uploadFiles = useCallback(
    async (files: File[]) => {
      const toAdd = files.slice(0, MAX_FILES - attachments.length);
      for (const file of toAdd) {
        if (file.size > MAX_SIZE_MB * 1024 * 1024) {
          continue;
        }
        const contentType = file.type || 'application/octet-stream';
        const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;

        onAttachmentsChange((prev) => [
          ...prev,
          {
            id,
            filename: file.name,
            contentType,
            size: file.size,
            status: 'uploading' as const,
          },
        ]);

        try {
          const { upload_url, key } = await getInitRefUploadUrl({
            filename: file.name,
            content_type: contentType,
          });

          const response = await fetch(upload_url, {
            method: 'PUT',
            headers: { 'Content-Type': contentType },
            body: file,
          });

          if (!response.ok) {
            throw new Error(`Upload failed: ${response.status}`);
          }

          onAttachmentsChange((prev) =>
            prev.map((a) =>
              a.id === id ? { ...a, key, status: 'uploaded' as const } : a
            )
          );
        } catch (error) {
          onAttachmentsChange((prev) =>
            prev.map((a) =>
              a.id === id
                ? {
                    ...a,
                    status: 'failed' as const,
                    error: error instanceof Error ? error.message : 'Upload failed',
                  }
                : a
            )
          );
        }
      }
    },
    [attachments.length, onAttachmentsChange]
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      if (!canAddMore || hasUploading) return;
      const files = Array.from(e.dataTransfer.files);
      uploadFiles(files);
    },
    [canAddMore, hasUploading, uploadFiles]
  );

  const handleFileInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files ? Array.from(e.target.files) : [];
      uploadFiles(files);
      e.target.value = '';
    },
    [uploadFiles]
  );

  return (
    <div className="space-y-3">
      <label className="block text-sm font-bold text-card-foreground">
        Reference Files (optional)
      </label>
      <p className="text-xs text-muted-foreground">
        Upload project samples, mockups, or specs. Agent will read them to scaffold better. Max {MAX_FILES} files, {MAX_SIZE_MB}MB each. ZIP, images, PDF, MD, JSON.
      </p>

      {canAddMore && (
        <div
          onDragOver={(e) => e.preventDefault()}
          onDrop={handleDrop}
          className="border-2 border-dashed border-border rounded-lg p-6 text-center hover:border-primary/50 transition-colors cursor-pointer"
          onClick={() =>
            document.getElementById('ref-files-input')?.click()
          }
        >
          <input
            id="ref-files-input"
            type="file"
            multiple
            accept={ALLOWED_TYPES.join(',')}
            onChange={handleFileInput}
            className="hidden"
          />
          <span className="material-symbols-outlined text-3xl text-muted-foreground mb-2 block">
            upload_file
          </span>
          <p className="text-sm text-muted-foreground">
            Drop files or click to select
          </p>
        </div>
      )}

      {attachments.length > 0 && (
        <div className="space-y-2">
          {attachments.map((a) => (
            <div
              key={a.id}
              className="flex items-center gap-3 p-2 rounded-lg border border-border bg-muted"
            >
              <span
                className={`material-symbols-outlined ${
                  a.status === 'uploaded'
                    ? 'text-emerald-500'
                    : a.status === 'failed'
                    ? 'text-red-500'
                    : 'text-muted-foreground animate-pulse'
                }`}
              >
                {a.status === 'uploaded'
                  ? 'check_circle'
                  : a.status === 'failed'
                  ? 'error'
                  : 'hourglass_empty'}
              </span>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium truncate">{a.filename}</p>
                <p className="text-xs text-muted-foreground">
                  {(a.size / 1024).toFixed(1)} KB
                  {a.status === 'failed' && a.error && ` • ${a.error}`}
                </p>
              </div>
              <button
                type="button"
                onClick={() => removeAttachment(a.id)}
                className="p-1 rounded hover:bg-red-500/20 text-muted-foreground hover:text-red-500"
                title="Remove"
              >
                <span className="material-symbols-outlined text-lg">close</span>
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export type { RefAttachment };

export function getReferenceKeys(attachments: RefAttachment[]): string[] {
  return attachments
    .filter((a) => a.status === 'uploaded' && a.key)
    .map((a) => a.key!);
}
