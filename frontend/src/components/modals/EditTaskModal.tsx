import { useEffect, useMemo, useRef, useState, type ChangeEvent, type DragEvent } from 'react';
import { TaskMetadataGrid } from './create-task/TaskMetadataGrid';
import { useProjectMembers } from '../../hooks/useProjectMembers';
import { useSprints } from '../../hooks/useSprints';
import { updateTaskMetadata, getTask, getTaskAttachmentUploadUrl } from '../../api/tasks';
import { updateTask as updateTaskApi } from '../../api/generated/tasks/tasks';
import { createTaskAttemptFromEdit } from '../../api/taskAttempts';
import type { UpdateTaskRequestDoc } from '../../api/generated/models';
import type { KanbanTask } from '../../types/project';
import type { TaskType } from '../../shared/types';
import { logger } from '@/lib/logger';

interface PendingAttachment {
  id: string;
  filename: string;
  contentType: string;
  size: number;
  key?: string;
  status: 'uploading' | 'uploaded' | 'failed';
  error?: string;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

type TaskTypeValue = Exclude<TaskType, 'init'>;
type TaskPriorityValue = 'low' | 'medium' | 'high' | 'critical';

// Map KanbanTask status to API format (PascalCase)
const statusToApi = (status: KanbanTask['status']): string => {
  const map: Record<KanbanTask['status'], string> = {
    todo: 'Todo',
    in_progress: 'InProgress',
    in_review: 'InReview',
    done: 'Done',
  };
  return map[status] ?? 'Todo';
};

interface EditTaskModalProps {
  isOpen: boolean;
  onClose: () => void;
  task: KanbanTask;
  projectId: string;
  onSuccess?: (newAttemptId?: string) => void;
}

export function EditTaskModal({
  isOpen,
  onClose,
  task,
  projectId,
  onSuccess,
}: EditTaskModalProps) {
  const [title, setTitle] = useState(task.title || '');
  const [description, setDescription] = useState(task.description || '');
  const [type, setType] = useState<TaskTypeValue>(task.type || 'feature');
  const [priority, setPriority] = useState<TaskPriorityValue>(task.priority || 'medium');
  const [assignee, setAssignee] = useState('');
  const [sprint, setSprint] = useState('');
  const [autoStart, setAutoStart] = useState(true);
  const [attachments, setAttachments] = useState<PendingAttachment[]>([]);
  const [isDragOver, setIsDragOver] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const attachmentInputRef = useRef<HTMLInputElement | null>(null);

  const { members } = useProjectMembers(projectId);
  const { sprints } = useSprints(projectId);
  const memberOptions = members.map((m) => ({ id: m.id, name: m.name }));

  useEffect(() => {
    if (!isOpen) return;
    setTitle(task.title || '');
    setDescription(task.description || '');
    setType((task.type || 'feature') as TaskTypeValue);
    setPriority((task.priority || 'medium') as TaskPriorityValue);
    setAttachments([]);
    setSubmitError(null);
  }, [isOpen, task]);

  const setAttachmentState = (id: string, updater: (item: PendingAttachment) => PendingAttachment) => {
    setAttachments((prev) =>
      prev.map((item) => (item.id === id ? updater(item) : item))
    );
  };

  const uploadedAttachments = useMemo(
    () =>
      attachments
        .filter((a): a is PendingAttachment & { key: string } => a.status === 'uploaded' && !!a.key)
        .map((a) => ({
          key: a.key,
          filename: a.filename,
          content_type: a.contentType,
          size: a.size,
          uploaded_at: new Date().toISOString(),
        })),
    [attachments]
  );

  const hasUploadingAttachments = attachments.some((a) => a.status === 'uploading');

  const uploadFiles = async (files: File[]) => {
    for (const file of files) {
      const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
      const contentType = file.type || 'application/octet-stream';

      setAttachments((prev) => [
        ...prev,
        { id, filename: file.name, contentType, size: file.size, status: 'uploading' },
      ]);

      try {
        const { upload_url, key } = await getTaskAttachmentUploadUrl({
          project_id: projectId,
          filename: file.name,
          content_type: contentType,
        });

        const uploadRes = await fetch(upload_url, {
          method: 'PUT',
          headers: { 'Content-Type': contentType },
          body: file,
        });

        if (!uploadRes.ok) throw new Error(`Upload failed status ${uploadRes.status}`);

        setAttachmentState(id, (item) => ({ ...item, key, status: 'uploaded', error: undefined }));
      } catch (error) {
        logger.error('Attachment upload failed:', error);
        setAttachmentState(id, (item) => ({
          ...item,
          status: 'failed',
          error: 'Upload failed',
        }));
      }
    }
  };

  const handleFileInputChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files ? Array.from(event.target.files) : [];
    event.target.value = '';
    if (files.length === 0) return;
    await uploadFiles(files);
  };

  const handleDrop = async (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setIsDragOver(false);
    const files = Array.from(event.dataTransfer.files || []);
    if (files.length === 0) return;
    await uploadFiles(files);
  };

  useEffect(() => {
    const activeSprint = sprints.find((s) => s.status === 'active');
    if (activeSprint && !sprint) setSprint(activeSprint.id);
    else if (sprints[0] && !sprints.some((s) => s.id === sprint)) setSprint(sprints[0].id);
  }, [sprints, sprint]);

  if (!isOpen) return null;

  const isFormValid = Boolean(title.trim()) && !hasUploadingAttachments;

  const handleSave = async () => {
    if (!isFormValid || isSaving) return;
    setIsSaving(true);
    setSubmitError(null);

    try {
      const updatePayload: UpdateTaskRequestDoc = {
        status: statusToApi(task.status),
        title: title.trim(),
        description: description.trim() || null,
        assigned_to: assignee || null,
        sprint_id: sprint || null,
      };
      await updateTaskApi(task.id, updatePayload);

      // Merge priority/type/attachments into existing metadata (PUT replaces entire metadata)
      const existing = await getTask(task.id);
      const currentMeta = existing.metadata ?? {};
      const existingAttachments = Array.isArray(currentMeta.attachments)
        ? (currentMeta.attachments as Array<{ key: string; filename: string; content_type: string; size: number }>)
        : [];
      const deduped = new Map<string, unknown>();
      existingAttachments.forEach((a) => deduped.set(a.key, a));
      uploadedAttachments.forEach((a) => deduped.set(a.key, a));
      const mergedAttachments = Array.from(deduped.values());
      const merged = {
        ...currentMeta,
        priority,
        task_type: type,
        attachments: mergedAttachments,
        attachments_count: mergedAttachments.length,
      };
      await updateTaskMetadata(task.id, merged);

      if (autoStart) {
        const newAttempt = await createTaskAttemptFromEdit(task.id);
        onSuccess?.(newAttempt.id);
      } else {
        onSuccess?.();
      }
      onClose();
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : 'Failed to update task');
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px] transition-opacity"
        onClick={onClose}
      />
      <div className="relative w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        <div className="px-6 py-5 border-b border-border flex justify-between items-center bg-muted">
          <div>
            <h2 className="text-lg font-bold text-card-foreground">Edit Task</h2>
            <p className="text-sm text-muted-foreground">Update task details</p>
          </div>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-card-foreground transition-colors"
          >
            <span className="material-symbols-outlined">close</span>
          </button>
        </div>

        <div className="p-6 overflow-y-auto flex flex-col gap-5">
          <div>
            <label className="block text-sm font-bold text-card-foreground mb-1.5">
              Task Title <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="e.g. Implement refresh token rotation"
              className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
            />
          </div>

          <div>
            <label className="block text-sm font-bold text-card-foreground mb-1.5">Description</label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Describe the task..."
              rows={4}
              className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground resize-none"
            />
          </div>

          <TaskMetadataGrid
            type={type}
            priority={priority}
            assignee={assignee}
            sprint={sprint}
            users={memberOptions}
            sprints={sprints}
            onTypeChange={setType}
            onPriorityChange={setPriority}
            onAssigneeChange={setAssignee}
            onSprintChange={setSprint}
          />

          <div className="space-y-3">
            <label className="block text-sm font-bold text-card-foreground">Reference files</label>
            <input
              ref={attachmentInputRef}
              type="file"
              multiple
              className="hidden"
              onChange={(e) => void handleFileInputChange(e)}
            />
            <div
              className={`border border-dashed rounded-lg p-4 flex flex-col items-center justify-center text-center transition-colors cursor-pointer ${
                isDragOver ? 'border-primary bg-primary/10' : 'border-border hover:bg-muted'
              }`}
              onClick={() => attachmentInputRef.current?.click()}
              onDragOver={(e) => { e.preventDefault(); setIsDragOver(true); }}
              onDragLeave={() => setIsDragOver(false)}
              onDrop={(e) => void handleDrop(e)}
            >
              <span className="material-symbols-outlined text-muted-foreground mb-2">cloud_upload</span>
              <p className="text-xs font-medium text-muted-foreground">
                Drop files to attach, or <span className="text-primary underline">browse</span>
              </p>
            </div>
            {attachments.length > 0 && (
              <div className="space-y-2">
                {attachments.map((a) => (
                  <div
                    key={a.id}
                    className="rounded-md border border-border bg-muted px-3 py-2 flex items-center justify-between gap-3"
                  >
                    <div className="min-w-0">
                      <p className="text-sm text-card-foreground truncate">{a.filename}</p>
                      <p className="text-xs text-muted-foreground">
                        {formatBytes(a.size)}
                        {a.status === 'failed' && a.error ? ` • ${a.error}` : ''}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      {a.status === 'uploading' && (
                        <span className="w-4 h-4 border-2 border-muted-foreground/30 border-t-muted-foreground rounded-full animate-spin" />
                      )}
                      {a.status === 'uploaded' && (
                        <span className="material-symbols-outlined text-green-500 text-[18px]">check_circle</span>
                      )}
                      {a.status === 'failed' && (
                        <span className="material-symbols-outlined text-red-500 text-[18px]">error</span>
                      )}
                      <button
                        type="button"
                        onClick={() => setAttachments((prev) => prev.filter((x) => x.id !== a.id))}
                        className="text-muted-foreground hover:text-card-foreground"
                        disabled={a.status === 'uploading'}
                        title="Remove"
                      >
                        <span className="material-symbols-outlined text-[18px]">close</span>
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="rounded-lg border border-border bg-muted/50 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-sm font-medium text-card-foreground">Auto start</p>
                <p className="text-xs text-muted-foreground">
                  Create new attempt and run agent after saving (cleans up old worktree & MR)
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={autoStart}
                onClick={() => setAutoStart((prev) => !prev)}
                className={`relative inline-flex h-5 w-10 items-center rounded-full border transition-colors ${
                  autoStart ? 'bg-emerald-500 border-emerald-400' : 'bg-slate-700 border-slate-500'
                }`}
              >
                <span
                  className={`inline-block h-3.5 w-3.5 rounded-full bg-white shadow-sm transition-transform ${
                    autoStart ? 'translate-x-[21px]' : 'translate-x-[2px]'
                  }`}
                />
              </button>
            </div>
          </div>

          {submitError && (
            <div className="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-400">
              {submitError}
            </div>
          )}
        </div>

        <div className="px-6 py-4 border-t border-border bg-muted flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={!isFormValid || isSaving}
            className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? (
              <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <span className="material-symbols-outlined text-[18px]">save</span>
            )}
            {isSaving ? 'Saving...' : 'Save Changes'}
          </button>
        </div>
      </div>
    </div>
  );
}
