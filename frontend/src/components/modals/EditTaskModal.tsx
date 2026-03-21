import { useEffect, useMemo, useRef, useState, type ChangeEvent, type DragEvent } from 'react';
import { TaskDocumentFields } from './create-task/TaskDocumentFields';
import { TaskMetadataGrid } from './create-task/TaskMetadataGrid';
import { useProjectMembers } from '../../hooks/useProjectMembers';
import { useSprints } from '../../hooks/useSprints';
import { getTask, updateTask, updateTaskMetadata } from '../../api/tasks';
import { createTaskAttemptFromEdit } from '../../api/taskAttempts';
import {
  createTaskContext,
  createTaskContextAttachment,
  deleteTaskContext,
  deleteTaskContextAttachment,
  getTaskContexts,
  getTaskContextAttachmentUploadUrl,
  updateTaskContext,
  type TaskContextAttachment,
} from '../../api/taskContexts';
import type { KanbanTask } from '../../types/project';
import type { Task, TaskType } from '../../shared/types';
import {
  getTaskDocumentMetadata,
  type TaskDocumentFormat,
  type TaskDocumentKind,
} from '../../lib/taskDocuments';
import { logger } from '@/lib/logger';

interface PendingContextFile {
  id: string;
  filename: string;
  contentType: string;
  size: number;
  file: File;
  status: 'pending' | 'uploading' | 'uploaded' | 'failed';
  attachmentId?: string;
  storageKey?: string;
  error?: string;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function buildTaskDocumentMetadata(options: {
  taskTitle: string;
  documentTitle: string;
  documentKind: TaskDocumentKind;
  documentFormat: TaskDocumentFormat;
  documentSourceUrl: string;
  documentFigmaUrl: string;
  documentFigmaNodeId: string;
}): Record<string, unknown> {
  const document: Record<string, unknown> = {
    kind: options.documentKind,
    format: options.documentFormat,
    preview_mode: 'document',
    publish_policy: 'final_on_done',
    title: options.documentTitle.trim() || options.taskTitle.trim(),
  };

  if (options.documentSourceUrl.trim()) {
    document.source_url = options.documentSourceUrl.trim();
  }
  if (options.documentFigmaUrl.trim()) {
    document.figma_url = options.documentFigmaUrl.trim();
  }
  if (options.documentFigmaNodeId.trim()) {
    document.figma_node_id = options.documentFigmaNodeId.trim();
  }

  return document;
}

type TaskTypeValue = Exclude<TaskType, 'init'>;
type TaskPriorityValue = 'low' | 'medium' | 'high' | 'critical';

interface EditTaskModalProps {
  isOpen: boolean;
  onClose: () => void;
  task: KanbanTask;
  projectId: string;
  onSuccess?: (newAttemptId?: string) => void;
}

function normalizeEditableTaskType(taskType: string | null | undefined): TaskTypeValue {
  if (!taskType || taskType === 'init') return 'chore';
  return taskType as TaskTypeValue;
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
  const [type, setType] = useState<TaskTypeValue>(normalizeEditableTaskType(task.type));
  const [originalTaskType, setOriginalTaskType] = useState<Task['task_type']>('feature');
  const [typeTouched, setTypeTouched] = useState(false);
  const [priority, setPriority] = useState<TaskPriorityValue>(task.priority || 'medium');
  const [assignee, setAssignee] = useState('');
  const [sprint, setSprint] = useState('');
  const [autoStart, setAutoStart] = useState(true);
  const [taskMetadata, setTaskMetadata] = useState<Record<string, unknown>>({});
  const [documentTitle, setDocumentTitle] = useState('');
  const [documentKind, setDocumentKind] = useState<TaskDocumentKind>('other');
  const [documentFormat, setDocumentFormat] = useState<TaskDocumentFormat>('markdown');
  const [documentSourceUrl, setDocumentSourceUrl] = useState('');
  const [documentFigmaUrl, setDocumentFigmaUrl] = useState('');
  const [documentFigmaNodeId, setDocumentFigmaNodeId] = useState('');

  const [taskContextId, setTaskContextId] = useState<string | null>(null);
  const [taskContextTitle, setTaskContextTitle] = useState('');
  const [taskContextContent, setTaskContextContent] = useState('');
  const [existingContextAttachments, setExistingContextAttachments] = useState<TaskContextAttachment[]>([]);
  const [deletedContextAttachments, setDeletedContextAttachments] = useState<TaskContextAttachment[]>([]);
  const [pendingContextFiles, setPendingContextFiles] = useState<PendingContextFile[]>([]);
  const [additionalContextCount, setAdditionalContextCount] = useState(0);
  const [additionalAttachmentCount, setAdditionalAttachmentCount] = useState(0);

  const [isDragOver, setIsDragOver] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isLoadingTaskData, setIsLoadingTaskData] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const attachmentInputRef = useRef<HTMLInputElement | null>(null);

  const { members } = useProjectMembers(projectId);
  const { sprints } = useSprints(projectId);
  const memberOptions = useMemo(
    () => members.map((member) => ({ id: member.id, name: member.name })),
    [members]
  );
  const isDocsTask = type === 'docs';

  useEffect(() => {
    if (!isOpen) return;

    let cancelled = false;

    const loadTaskData = async () => {
      setIsLoadingTaskData(true);
      setSubmitError(null);
      setPendingContextFiles([]);
      setDeletedContextAttachments([]);

      try {
        const [taskDetails, contexts] = await Promise.all([getTask(task.id), getTaskContexts(task.id)]);
        if (cancelled) return;

        setTitle(taskDetails.title || '');
        setDescription(taskDetails.description || '');
        setOriginalTaskType(taskDetails.task_type);
        setType(normalizeEditableTaskType(taskDetails.task_type));
        setTypeTouched(false);
        setPriority((taskDetails.metadata?.priority as TaskPriorityValue) || 'medium');
        setAssignee(taskDetails.assigned_to || '');
        setSprint(taskDetails.sprint_id || '');
        setAutoStart(true);
        setTaskMetadata((taskDetails.metadata as Record<string, unknown>) || {});

        const documentMetadata = getTaskDocumentMetadata(
          taskDetails.task_type,
          taskDetails.title,
          (taskDetails.metadata as Record<string, unknown>) || {}
        );
        setDocumentTitle(documentMetadata?.title ?? taskDetails.title ?? '');
        setDocumentKind(documentMetadata?.kind ?? 'other');
        setDocumentFormat(documentMetadata?.format ?? 'markdown');
        setDocumentSourceUrl(documentMetadata?.sourceUrl ?? '');
        setDocumentFigmaUrl(documentMetadata?.figmaUrl ?? '');
        setDocumentFigmaNodeId(documentMetadata?.figmaNodeId ?? '');

        const sortedContexts = [...contexts].sort((left, right) => {
          if (left.sort_order !== right.sort_order) {
            return left.sort_order - right.sort_order;
          }

          return new Date(left.created_at).getTime() - new Date(right.created_at).getTime();
        });
        const primaryContext = sortedContexts[0];
        const additionalContexts = sortedContexts.slice(1);

        setTaskContextId(primaryContext?.id ?? null);
        setTaskContextTitle(primaryContext?.title ?? '');
        setTaskContextContent(primaryContext?.raw_content ?? '');
        setExistingContextAttachments(primaryContext?.attachments ?? []);
        setAdditionalContextCount(additionalContexts.length);
        setAdditionalAttachmentCount(
          additionalContexts.reduce((sum, context) => sum + context.attachments.length, 0)
        );
      } catch (error) {
        logger.error('Failed to load task edit context:', error);
        if (!cancelled) {
          setSubmitError(
            error instanceof Error ? error.message : 'Failed to load task details'
          );
        }
      } finally {
        if (!cancelled) {
          setIsLoadingTaskData(false);
        }
      }
    };

    void loadTaskData();

    return () => {
      cancelled = true;
    };
  }, [isOpen, task.id]);

  useEffect(() => {
    if (!isOpen || !sprints.length) return;
    if (sprint && sprints.some((item) => item.id === sprint)) return;

    const activeSprint = sprints.find((item) => item.status === 'active');
    setSprint(activeSprint?.id || sprints[0]?.id || '');
  }, [isOpen, sprint, sprints]);

  useEffect(() => {
    if (!isOpen || !assignee || members.length === 0) return;
    if (!members.some((member) => member.id === assignee)) {
      setAssignee('');
    }
  }, [assignee, isOpen, members]);

  const setPendingContextFileState = (
    id: string,
    updater: (item: PendingContextFile) => PendingContextFile
  ) => {
    setPendingContextFiles((prev) =>
      prev.map((item) => (item.id === id ? updater(item) : item))
    );
  };

  const addContextFiles = (files: File[]) => {
    for (const file of files) {
      const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
      const contentType = file.type || 'application/octet-stream';

      setPendingContextFiles((prev) => [
        ...prev,
        {
          id,
          filename: file.name,
          contentType,
          size: file.size,
          file,
          status: 'pending',
        },
      ]);
    }
  };

  const uploadTaskContextFiles = async (
    taskId: string,
    contextId: string,
    files: PendingContextFile[]
  ): Promise<number> => {
    let uploadedCount = 0;

    for (const file of files) {
      if (file.status === 'uploaded' && file.attachmentId) {
        continue;
      }

      setPendingContextFileState(file.id, (item) => ({
        ...item,
        status: 'uploading',
        error: undefined,
      }));

      try {
        const { upload_url, key } = await getTaskContextAttachmentUploadUrl(taskId, {
          filename: file.filename,
          content_type: file.contentType,
        });

        const uploadRes = await fetch(upload_url, {
          method: 'PUT',
          headers: { 'Content-Type': file.contentType },
          body: file.file,
        });

        if (!uploadRes.ok) {
          throw new Error(`Upload failed with status ${uploadRes.status}`);
        }

        const attachment = await createTaskContextAttachment(taskId, contextId, {
          storage_key: key,
          filename: file.filename,
          content_type: file.contentType,
          size_bytes: file.size,
          checksum: null,
        });

        uploadedCount += 1;
        setPendingContextFileState(file.id, (item) => ({
          ...item,
          status: 'uploaded',
          attachmentId: attachment.id,
          storageKey: key,
          error: undefined,
        }));
      } catch (error) {
        logger.error('Task context attachment upload failed:', error);
        setPendingContextFileState(file.id, (item) => ({
          ...item,
          status: 'failed',
          error: 'Upload failed',
        }));
        throw error;
      }
    }

    return uploadedCount;
  };

  const handleFileInputChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files ? Array.from(event.target.files) : [];
    event.target.value = '';
    if (files.length === 0) return;
    addContextFiles(files);
  };

  const handleDrop = async (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setIsDragOver(false);
    const files = Array.from(event.dataTransfer.files || []);
    if (files.length === 0) return;
    addContextFiles(files);
  };

  const handleRemoveExistingAttachment = (attachment: TaskContextAttachment) => {
    setExistingContextAttachments((prev) => prev.filter((item) => item.id !== attachment.id));
    setDeletedContextAttachments((prev) => [...prev, attachment]);
  };

  const handleRemovePendingFile = async (file: PendingContextFile) => {
    if (file.status === 'uploading') return;

    if (file.status === 'uploaded' && taskContextId && file.attachmentId) {
      try {
        await deleteTaskContextAttachment(task.id, taskContextId, file.attachmentId);
      } catch (error) {
        logger.error('Failed to remove uploaded task context attachment:', error);
        setSubmitError(
          error instanceof Error ? error.message : 'Failed to remove uploaded attachment'
        );
        return;
      }
    }

    setPendingContextFiles((prev) => prev.filter((item) => item.id !== file.id));
  };

  if (!isOpen) return null;

  const uploadedPendingAttachmentCount = pendingContextFiles.filter(
    (file) => file.status === 'uploaded' && file.attachmentId
  ).length;
  const hasUploadingContextFiles = pendingContextFiles.some(
    (file) => file.status === 'uploading'
  );
  const isFormValid = Boolean(title.trim()) && !hasUploadingContextFiles && !isLoadingTaskData;

  const handleSave = async () => {
    if (!isFormValid || isSaving) return;
    setIsSaving(true);
    setSubmitError(null);

    try {
      const nextTaskType =
        originalTaskType === 'init' && !typeTouched && type === 'chore' ? undefined : type;
      const effectiveTaskType = nextTaskType ?? originalTaskType;
      const isNextDocsTask = effectiveTaskType === 'docs';

      await updateTask(task.id, {
        status: task.status,
        title: title.trim(),
        description: description.trim() || null,
        task_type: nextTaskType ?? undefined,
        assigned_to: assignee || null,
        sprint_id: sprint || null,
      });

      const hasTaskContext =
        Boolean((isNextDocsTask ? '' : taskContextTitle).trim()) ||
        Boolean(taskContextContent.trim()) ||
        existingContextAttachments.length > 0 ||
        pendingContextFiles.length > 0;

      let currentContextId = taskContextId;
      let primaryAttachmentCount = 0;

      if (!hasTaskContext) {
        if (currentContextId) {
          await deleteTaskContext(task.id, currentContextId);
          currentContextId = null;
          setTaskContextId(null);
        }
      } else {
        if (currentContextId) {
          await updateTaskContext(task.id, currentContextId, {
            title: isNextDocsTask ? null : taskContextTitle.trim() || null,
            content_type: 'text/markdown',
            raw_content: taskContextContent.trim(),
            sort_order: 0,
          });
        } else {
          const createdContext = await createTaskContext(task.id, {
            title: isNextDocsTask ? null : taskContextTitle.trim() || null,
            content_type: 'text/markdown',
            raw_content: taskContextContent.trim(),
            source: 'user',
            sort_order: 0,
          });

          currentContextId = createdContext.id;
          setTaskContextId(createdContext.id);
        }

        if (currentContextId) {
          for (const attachment of deletedContextAttachments) {
            await deleteTaskContextAttachment(task.id, currentContextId, attachment.id);
          }

          const newlyUploadedAttachmentCount = await uploadTaskContextFiles(
            task.id,
            currentContextId,
            pendingContextFiles
          );

          primaryAttachmentCount =
            existingContextAttachments.length +
            uploadedPendingAttachmentCount +
            newlyUploadedAttachmentCount;
        }
      }

      const {
        attachments: _legacyAttachments,
        attachments_count: _legacyAttachmentsCount,
        priority: _legacyPriority,
        task_type: _legacyTaskType,
        ...remainingMetadata
      } = taskMetadata;

      const nextMetadata: Record<string, unknown> = {
        ...remainingMetadata,
        priority,
        attachments_count: additionalAttachmentCount + primaryAttachmentCount,
      };

      if (isNextDocsTask) {
        const execution = isPlainObject(nextMetadata.execution)
          ? { ...nextMetadata.execution }
          : {};
        execution.no_code_changes = true;
        execution.run_build_and_tests = false;
        execution.auto_deploy = false;
        nextMetadata.execution = execution;
        nextMetadata.document = buildTaskDocumentMetadata({
          taskTitle: title,
          documentTitle,
          documentKind,
          documentFormat,
          documentSourceUrl,
          documentFigmaUrl,
          documentFigmaNodeId,
        });
      } else {
        delete nextMetadata.document;
        if (isPlainObject(nextMetadata.execution)) {
          const execution = { ...nextMetadata.execution };
          delete execution.no_code_changes;
          if (originalTaskType === 'docs') {
            delete execution.run_build_and_tests;
            delete execution.auto_deploy;
          }

          if (Object.keys(execution).length === 0) {
            delete nextMetadata.execution;
          } else {
            nextMetadata.execution = execution;
          }
        }
      }

      await updateTaskMetadata(task.id, nextMetadata);

      if (autoStart) {
        const newAttempt = await createTaskAttemptFromEdit(task.id);
        onSuccess?.(newAttempt.id);
      } else {
        onSuccess?.();
      }

      onClose();
    } catch (error) {
      logger.error('Failed to update task:', error);
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
            <p className="text-sm text-muted-foreground">Update task details and task context</p>
          </div>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-card-foreground transition-colors"
          >
            <span className="material-symbols-outlined">close</span>
          </button>
        </div>

        <div className="p-6 overflow-y-auto flex flex-col gap-5">
          {isLoadingTaskData && (
            <div className="rounded-lg border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
              Loading current task details...
            </div>
          )}

          <div>
            <label className="block text-sm font-bold text-card-foreground mb-1.5">
              Task Title <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="e.g. Implement refresh token rotation"
              className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
            />
          </div>

          <div>
            <label className="block text-sm font-bold text-card-foreground mb-1.5">
              Description
            </label>
            <textarea
              value={description}
              onChange={(event) => setDescription(event.target.value)}
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
            onTypeChange={(value) => {
              setTypeTouched(true);
              setType(value);
            }}
            onPriorityChange={setPriority}
            onAssigneeChange={setAssignee}
            onSprintChange={setSprint}
          />

          <div className="space-y-4 rounded-xl border border-border bg-muted/30 p-4">
            <div>
              <h3 className="text-sm font-bold text-card-foreground">
                {isDocsTask ? 'Document Content' : 'Task Context'}
              </h3>
              <p className="text-xs text-muted-foreground mt-1">
                {isDocsTask
                  ? 'Keep the final document metadata, notes, and supporting assets in sync with this task.'
                  : 'Keep the agent-facing notes and reference files in sync with this task.'}
              </p>
              {additionalContextCount > 0 && (
                <p className="text-xs text-amber-600 dark:text-amber-300 mt-2">
                  This editor updates the primary context block only. {additionalContextCount}{' '}
                  additional context block{additionalContextCount === 1 ? '' : 's'} and{' '}
                  {additionalAttachmentCount} attachment
                  {additionalAttachmentCount === 1 ? '' : 's'} remain unchanged.
                </p>
              )}
            </div>

            {isDocsTask ? (
              <TaskDocumentFields
                documentTitle={documentTitle}
                onDocumentTitleChange={setDocumentTitle}
                documentKind={documentKind}
                onDocumentKindChange={setDocumentKind}
                documentFormat={documentFormat}
                onDocumentFormatChange={setDocumentFormat}
                documentSourceUrl={documentSourceUrl}
                onDocumentSourceUrlChange={setDocumentSourceUrl}
                documentFigmaUrl={documentFigmaUrl}
                onDocumentFigmaUrlChange={setDocumentFigmaUrl}
                documentFigmaNodeId={documentFigmaNodeId}
                onDocumentFigmaNodeIdChange={setDocumentFigmaNodeId}
                documentContent={taskContextContent}
                onDocumentContentChange={setTaskContextContent}
              />
            ) : (
              <>
                <div>
                  <label className="block text-sm font-bold text-card-foreground mb-1.5">
                    Context Title
                  </label>
                  <input
                    type="text"
                    value={taskContextTitle}
                    onChange={(event) => setTaskContextTitle(event.target.value)}
                    placeholder="e.g. Login screen constraints"
                    className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
                  />
                </div>

                <div>
                  <label className="block text-sm font-bold text-card-foreground mb-1.5">
                    Context Notes
                  </label>
                  <textarea
                    value={taskContextContent}
                    onChange={(event) => setTaskContextContent(event.target.value)}
                    placeholder="Describe the required copy, design constraints, edge cases, or debugging context."
                    rows={5}
                    className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground resize-none"
                  />
                </div>
              </>
            )}

            <input
              ref={attachmentInputRef}
              type="file"
              multiple
              className="hidden"
              onChange={(event) => {
                void handleFileInputChange(event);
              }}
            />

            <div
              className={`border border-dashed rounded-lg p-4 flex flex-col items-center justify-center text-center transition-colors cursor-pointer ${
                isDragOver ? 'border-primary bg-primary/10' : 'border-border hover:bg-card'
              }`}
              onClick={() => attachmentInputRef.current?.click()}
              onDragOver={(event) => {
                event.preventDefault();
                setIsDragOver(true);
              }}
              onDragLeave={() => setIsDragOver(false)}
              onDrop={(event) => {
                void handleDrop(event);
              }}
            >
              <span className="material-symbols-outlined text-muted-foreground mb-2">cloud_upload</span>
              <p className="text-xs font-medium text-muted-foreground">
                {isDocsTask ? 'Drop document assets, or ' : 'Drop reference files, or '}
                <span className="text-primary underline">browse</span>
              </p>
            </div>

            {existingContextAttachments.length > 0 && (
              <div className="space-y-2">
                <p className="text-xs font-bold uppercase tracking-wide text-muted-foreground">
                  Existing attachments
                </p>
                {existingContextAttachments.map((attachment) => (
                  <div
                    key={attachment.id}
                    className="rounded-md border border-border bg-muted px-3 py-2 flex items-center justify-between gap-3"
                  >
                    <div className="min-w-0">
                      <p className="text-sm text-card-foreground truncate">{attachment.filename}</p>
                      <p className="text-xs text-muted-foreground">
                        {formatBytes(attachment.size_bytes || 0)}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => handleRemoveExistingAttachment(attachment)}
                      className="text-muted-foreground hover:text-card-foreground"
                      title="Remove attachment"
                    >
                      <span className="material-symbols-outlined text-[18px]">close</span>
                    </button>
                  </div>
                ))}
              </div>
            )}

            {pendingContextFiles.length > 0 && (
              <div className="space-y-2">
                <p className="text-xs font-bold uppercase tracking-wide text-muted-foreground">
                  New attachments
                </p>
                {pendingContextFiles.map((file) => (
                  <div
                    key={file.id}
                    className="rounded-md border border-border bg-muted px-3 py-2 flex items-center justify-between gap-3"
                  >
                    <div className="min-w-0">
                      <p className="text-sm text-card-foreground truncate">{file.filename}</p>
                      <p className="text-xs text-muted-foreground">
                        {formatBytes(file.size)}
                        {file.status === 'pending' && ' • queued'}
                        {file.status === 'uploaded' && ' • uploaded'}
                        {file.status === 'failed' && file.error ? ` • ${file.error}` : ''}
                      </p>
                    </div>
                    <div className="flex items-center gap-2">
                      {file.status === 'uploading' && (
                        <span className="w-4 h-4 border-2 border-muted-foreground/30 border-t-muted-foreground rounded-full animate-spin" />
                      )}
                      {file.status === 'uploaded' && (
                        <span className="material-symbols-outlined text-green-500 text-[18px]">
                          check_circle
                        </span>
                      )}
                      {file.status === 'failed' && (
                        <span className="material-symbols-outlined text-red-500 text-[18px]">
                          error
                        </span>
                      )}
                      {file.status === 'pending' && (
                        <span className="material-symbols-outlined text-muted-foreground text-[18px]">
                          schedule
                        </span>
                      )}
                      <button
                        type="button"
                        onClick={() => {
                          void handleRemovePendingFile(file);
                        }}
                        className="text-muted-foreground hover:text-card-foreground"
                        disabled={file.status === 'uploading'}
                        title="Remove attachment"
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
                  Create a new attempt and run the agent after saving.
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
