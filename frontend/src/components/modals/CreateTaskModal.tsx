import { useEffect, useMemo, useRef, useState, type ChangeEvent, type DragEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { ProjectSelector } from './create-task/ProjectSelector';
import { TaskMetadataGrid } from './create-task/TaskMetadataGrid';
import { AIDescriptionField } from './create-task/AIDescriptionField';
import { useProjects } from '../../hooks/useProjects';
import { useSprints } from '../../hooks/useSprints';
import type { SprintDto } from '../../api/generated/models';
import type { ProjectMember } from '../../api/projects';
import { useProjectMembers } from '../../hooks/useProjectMembers';
import { useProjectSettings } from '../../hooks/useProjectSettings';
import { useSettings } from '../../hooks/useSettings';
import { createTask, deleteTask, updateTaskMetadata } from '../../api/tasks';
import { createTaskAttempt } from '../../api/taskAttempts';
import {
    createTaskContext,
    createTaskContextAttachment,
    getTaskContextAttachmentUploadUrl,
} from '../../api/taskContexts';
import type { TaskType } from '../../shared/types';
import type { RepositoryContext } from '../../types/repository';
import {
    getRepositoryAccessSummary,
    isRepositoryReadOnly,
    normalizeRepositoryContext,
} from '../../utils/repositoryAccess';
import { logger } from '@/lib/logger';
import { SourceControlSetupRequiredDialog } from './SourceControlSetupRequiredDialog';

type TaskTypeValue = Exclude<TaskType, 'init'>;
type TaskPriorityValue = 'low' | 'medium' | 'high' | 'critical';

interface PendingContextFile {
    id: string;
    filename: string;
    contentType: string;
    size: number;
    file: File;
    status: 'pending' | 'uploading' | 'uploaded' | 'failed';
    error?: string;
}

interface CreateTaskModalProps {
    isOpen: boolean;
    onClose: () => void;
    projectId?: string;
    projectName?: string;
    repositoryContext?: RepositoryContext;
    /** When provided (e.g. from ProjectDetailPage), avoids duplicate sprints fetch */
    sprints?: SprintDto[];
    /** When provided (e.g. from ProjectDetailPage), avoids duplicate members fetch */
    members?: ProjectMember[];
    navigateToProjectOnCreate?: boolean;
    onCreate?: (data: {
        projectId: string;
        title: string;
        description: string;
        priority: TaskPriorityValue;
        type: TaskTypeValue;
        assignee: string;
        sprint?: string;
        taskId?: string;
        autoStarted?: boolean;
    }) => void | Promise<void>;
}

function normalizeSprintStatus(status: string | null | undefined): string {
    if (!status) return '';
    const lower = status.toLowerCase();
    if (lower === 'planning') return 'planned';
    if (lower === 'completed') return 'closed';
    return lower;
}

function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function CreateTaskModal({
    isOpen,
    onClose,
    projectId,
    projectName,
    repositoryContext,
    sprints: sprintsProp,
    members: membersProp,
    navigateToProjectOnCreate = true,
    onCreate,
}: CreateTaskModalProps) {
    const { projects, apiProjects, loading: projectsLoading } = useProjects({ limit: 500 });
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const attachmentInputRef = useRef<HTMLInputElement | null>(null);

    const [isCreating, setIsCreating] = useState(false);
    const [isAiGenerating, setIsAiGenerating] = useState(false);
    const [isDragOver, setIsDragOver] = useState(false);
    const [submitError, setSubmitError] = useState<string | null>(null);
    const [showSetupDialog, setShowSetupDialog] = useState(false);

    const [selectedProject, setSelectedProject] = useState(projectId || '');
    const effectiveProjectId = projectId || selectedProject;

    const { sprints: sprintsFromHook } = useSprints(sprintsProp ? undefined : effectiveProjectId);
    const sprints = sprintsProp ?? sprintsFromHook;
    const { members: membersFromHook, loading: membersLoading } = useProjectMembers(
        membersProp ? undefined : effectiveProjectId,
    );
    const members = membersProp ?? membersFromHook;
    const { settings: projectSettings, loading: projectSettingsLoading } = useProjectSettings({
        projectId: effectiveProjectId || '',
        enabled: isOpen && Boolean(effectiveProjectId),
    });
    const { settings, loading: settingsLoading } = useSettings();

    const memberOptions = useMemo(() => members.map((member) => ({ id: member.id, name: member.name })), [members]);

    const [title, setTitle] = useState('');
    const [description, setDescription] = useState('');
    const [type, setType] = useState<TaskTypeValue>('feature');
    const [priority, setPriority] = useState<TaskPriorityValue>('medium');
    const [assignee, setAssignee] = useState('');
    const [sprint, setSprint] = useState('');

    const [autoStart, setAutoStart] = useState(false);
    const [requireReview, setRequireReview] = useState(false);
    const [requireReviewTouched, setRequireReviewTouched] = useState(false);
    const [autoDeploy, setAutoDeploy] = useState(false);

    const [taskContextFiles, setTaskContextFiles] = useState<PendingContextFile[]>([]);

    const selectedProjectRecord = useMemo(
        () => apiProjects.find((project) => project.id === effectiveProjectId),
        [apiProjects, effectiveProjectId],
    );
    const repositoryGuardEnabled = Boolean(
        effectiveProjectId && (repositoryContext || selectedProjectRecord?.repository_context),
    );
    const effectiveRepositoryContext = normalizeRepositoryContext(
        repositoryContext ?? selectedProjectRecord?.repository_context,
    );
    const repositoryReadOnly = repositoryGuardEnabled && isRepositoryReadOnly(effectiveRepositoryContext);
    const repositorySummary = getRepositoryAccessSummary(effectiveRepositoryContext);
    const sourceControlConfigured = Boolean(settings?.gitlab?.configured);
    const sourceControlSetupRequired = !settingsLoading && !sourceControlConfigured;
    const projectTaskPreviewEnabled = Boolean(
        projectSettings?.auto_deploy || projectSettings?.preview_enabled,
    );
    const taskPreviewLocked = !projectSettingsLoading && !projectTaskPreviewEnabled;

    const showProjectSelector = !projectId;
    const isFormValid = Boolean(title.trim()) && Boolean(effectiveProjectId);

    useEffect(() => {
        if (projectId) setSelectedProject(projectId);
    }, [projectId]);

    useEffect(() => {
        if (!isOpen) return;
        setSelectedProject(projectId || '');
        setTitle('');
        setDescription('');
        setType('feature');
        setPriority('medium');
        setAssignee('');
        setSprint('');
        setAutoStart(false);
        setRequireReview(true);
        setRequireReviewTouched(false);
        setAutoDeploy(false);
        setTaskContextFiles([]);
        setSubmitError(null);
        setShowSetupDialog(false);
    }, [isOpen, projectId]);

    useEffect(() => {
        if (!sprint || !sprints.some((item) => item.id === sprint)) {
            const activeSprint = sprints.find((item) => normalizeSprintStatus(item.status) === 'active');
            setSprint(activeSprint?.id || sprints[0]?.id || '');
        }
    }, [sprints, sprint]);

    useEffect(() => {
        if (!isOpen) return;
        setTaskContextFiles([]);
    }, [effectiveProjectId, isOpen]);

    useEffect(() => {
        if (!isOpen) return;
        setRequireReviewTouched(false);
        if (effectiveProjectId) {
            setRequireReview(true);
        }
    }, [effectiveProjectId, isOpen]);

    useEffect(() => {
        if (!isOpen || requireReviewTouched) return;
        if (projectSettings) {
            setRequireReview(projectSettings.require_review);
        }
    }, [isOpen, projectSettings, requireReviewTouched]);

    useEffect(() => {
        if (assignee && !members.some((member) => member.id === assignee)) {
            setAssignee('');
        }
    }, [assignee, members]);

    useEffect(() => {
        if (repositoryReadOnly && autoStart) {
            setAutoStart(false);
        }
    }, [autoStart, repositoryReadOnly]);

    if (!isOpen) return null;

    const getSelectedProjectName = () => {
        if (projectName) return projectName;
        const project = projects.find((item) => item.id === selectedProject);
        return project?.name || 'Select a project';
    };

    const handleGenerateAI = () => {
        setIsAiGenerating(true);
        setTimeout(() => {
            setDescription(
                'Implement the functionality as described in the task title. Ensure:\n\n1. Proper error handling\n2. Unit test coverage\n3. Documentation updates\n4. Code review checklist completed',
            );
            setIsAiGenerating(false);
        }, 1500);
    };

    const setContextFileState = (id: string, updater: (item: PendingContextFile) => PendingContextFile) => {
        setTaskContextFiles((prev) => prev.map((item) => (item.id === id ? updater(item) : item)));
    };

    const addContextFiles = (files: File[]) => {
        for (const file of files) {
            const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
            const contentType = file.type || 'application/octet-stream';

            setTaskContextFiles((prev) => [
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
        files: PendingContextFile[],
    ): Promise<number> => {
        let uploadedCount = 0;

        for (const file of files) {
            setContextFileState(file.id, (item) => ({
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
                    headers: {
                        'Content-Type': file.contentType,
                    },
                    body: file.file,
                });

                if (!uploadRes.ok) {
                    throw new Error(`Upload failed with status ${uploadRes.status}`);
                }

                await createTaskContextAttachment(taskId, contextId, {
                    storage_key: key,
                    filename: file.filename,
                    content_type: file.contentType,
                    size_bytes: file.size,
                    checksum: null,
                });

                uploadedCount += 1;
                setContextFileState(file.id, (item) => ({
                    ...item,
                    status: 'uploaded',
                    error: undefined,
                }));
            } catch (error) {
                logger.error('Task context attachment upload failed:', error);
                setContextFileState(file.id, (item) => ({
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

    const handleCreate = async () => {
        if (!isFormValid) return;
        if (sourceControlSetupRequired) {
            setShowSetupDialog(true);
            return;
        }
        setIsCreating(true);
        setSubmitError(null);

        let createdTaskId: string | null = null;

        try {
            const finalProjectId = effectiveProjectId;
            if (!finalProjectId) return;

            const baseMetadata = {
                priority,
                execution: {
                    require_review: requireReview,
                    run_build_and_tests: true,
                    auto_deploy: projectTaskPreviewEnabled && autoDeploy,
                },
                require_review: requireReview,
            };

            const task = await createTask({
                project_id: finalProjectId,
                title: title.trim(),
                description: description.trim() || undefined,
                task_type: type,
                assigned_to: assignee || undefined,
                sprint_id: sprint || undefined,
                metadata: baseMetadata,
            });
            createdTaskId = task.id;

            let uploadedAttachmentCount = 0;
            const hasTaskContext = taskContextFiles.length > 0;

            if (hasTaskContext) {
                const context = await createTaskContext(task.id, {
                    title: null,
                    content_type: 'text/markdown',
                    raw_content: '',
                    source: 'user',
                    sort_order: 0,
                });

                if (taskContextFiles.length > 0) {
                    uploadedAttachmentCount = await uploadTaskContextFiles(task.id, context.id, taskContextFiles);
                }
            }

            await updateTaskMetadata(task.id, {
                ...baseMetadata,
                attachments_count: uploadedAttachmentCount,
            });

            let autoStarted = false;
            if (autoStart) {
                try {
                    await createTaskAttempt(task.id);
                    autoStarted = true;
                } catch (error) {
                    logger.error('Task created but auto-start failed:', error);
                }
            }

            await Promise.all([
                queryClient.invalidateQueries({ queryKey: ['/api/v1/tasks'] }),
                queryClient.invalidateQueries({ queryKey: ['/api/v1/projects'] }),
                queryClient.invalidateQueries({ queryKey: ['/api/v1/dashboard'] }),
            ]);

            if (onCreate) {
                await onCreate({
                    projectId: finalProjectId,
                    title: title.trim(),
                    description,
                    priority,
                    type,
                    assignee,
                    sprint: sprint || undefined,
                    taskId: task.id,
                    autoStarted,
                });
            }

            onClose();
            if (navigateToProjectOnCreate) {
                navigate(`/projects/${finalProjectId}`);
            }
        } catch (error) {
            if (createdTaskId) {
                try {
                    await deleteTask(createdTaskId);
                } catch (rollbackError) {
                    logger.error('Failed to roll back task after context sync error:', rollbackError);
                }
            }
            logger.error('Failed to create task:', error);
            setSubmitError(error instanceof Error ? error.message : 'Failed to create task and sync task context');
        } finally {
            setIsCreating(false);
        }
    };

    const toggleCardClass = (enabled: boolean) =>
        `flex-1 min-w-0 text-left rounded-lg border p-3 transition-all ${
            enabled
                ? 'border-primary/70 bg-primary/10 shadow-sm shadow-primary/20'
                : 'border-border bg-card/50 hover:bg-card'
        }`;

    const toggleTitleClass = (enabled: boolean) => (enabled ? 'text-primary' : 'text-card-foreground');

    const toggleTrackClass = (enabled: boolean) =>
        `relative inline-flex h-5 w-10 items-center rounded-full border transition-colors ${
            enabled ? 'bg-emerald-500 border-emerald-400' : 'bg-slate-700 border-slate-500'
        }`;

    const toggleThumbClass = (enabled: boolean) =>
        `inline-block h-3.5 w-3.5 rounded-full bg-white shadow-sm shadow-black/30 transition-transform ${
            enabled ? 'translate-x-[21px]' : 'translate-x-[2px]'
        }`;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
            <div
                className="absolute inset-0 bg-black/70 backdrop-blur-[2px] transition-opacity"
                onClick={onClose}
            ></div>
            <div className="relative w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
                <div className="px-6 py-5 border-b border-border flex justify-between items-center bg-muted">
                    <div>
                        <h2 className="text-lg font-bold text-card-foreground">Create New Task</h2>
                        <p className="text-sm text-muted-foreground">
                            {showProjectSelector
                                ? 'Select a project and add a new task.'
                                : `Adding task to ${getSelectedProjectName()}`}
                        </p>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-muted-foreground hover:text-card-foreground transition-colors"
                    >
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                <div className="p-6 overflow-y-auto flex flex-col gap-5">
                    {showProjectSelector && (
                        <ProjectSelector
                            projects={projects}
                            selectedProject={selectedProject}
                            onProjectChange={setSelectedProject}
                            loading={projectsLoading}
                        />
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

                    <AIDescriptionField
                        value={description}
                        onChange={setDescription}
                        onGenerateAI={handleGenerateAI}
                        isGenerating={isAiGenerating}
                        titleProvided={title.trim().length > 0}
                    />

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

                    <div className="space-y-4 rounded-xl border border-border bg-muted/30 p-4">
                        <div>
                            <h3 className="text-sm font-bold text-card-foreground">Reference Files</h3>
                            <p className="text-xs text-muted-foreground mt-1">
                                Upload specs, screenshots, logs, or any files the agent should use for this task.
                            </p>
                        </div>

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
                                Drop reference files, or <span className="text-primary underline">browse</span>
                            </p>
                        </div>

                        {taskContextFiles.length > 0 && (
                            <div className="space-y-2">
                                {taskContextFiles.map((file) => (
                                    <div
                                        key={file.id}
                                        className="rounded-md border border-border bg-muted px-3 py-2 flex items-center justify-between gap-3"
                                    >
                                        <div className="min-w-0">
                                            <p className="text-sm text-card-foreground truncate">{file.filename}</p>
                                            <p className="text-xs text-muted-foreground">
                                                {formatBytes(file.size)}
                                                {file.status === 'pending' && ' • queued'}
                                                {file.status === 'failed' && file.error ? ` • ${file.error}` : ''}
                                            </p>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            {file.status === 'uploading' && (
                                                <span className="w-4 h-4 border-2 border-muted-foreground/30 border-t-muted-foreground rounded-full animate-spin"></span>
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
                                                onClick={() =>
                                                    setTaskContextFiles((prev) =>
                                                        prev.filter((item) => item.id !== file.id),
                                                    )
                                                }
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

                    {effectiveProjectId && repositoryReadOnly && (
                        <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-500/30 dark:bg-amber-500/10">
                            <div className="flex items-start gap-3">
                                <span className="material-symbols-outlined text-amber-600 dark:text-amber-300">
                                    lock
                                </span>
                                <div>
                                    <p className="text-sm font-semibold text-amber-900 dark:text-amber-100">
                                        {repositorySummary.title}
                                    </p>
                                    <p className="text-sm text-amber-800 dark:text-amber-200 mt-1">
                                        {repositorySummary.description}
                                    </p>
                                    <p className="text-xs text-amber-700 dark:text-amber-300 mt-2">
                                        {repositorySummary.action}
                                    </p>
                                </div>
                            </div>
                        </div>
                    )}

                    {sourceControlSetupRequired && (
                        <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-500/30 dark:bg-amber-500/10">
                            <div className="flex items-start gap-3">
                                <span className="material-symbols-outlined text-amber-600 dark:text-amber-300">
                                    settings_alert
                                </span>
                                <div>
                                    <p className="text-sm font-semibold text-amber-900 dark:text-amber-100">
                                        Source control is not configured
                                    </p>
                                    <p className="text-sm text-amber-800 dark:text-amber-200 mt-1">
                                        Configure GitLab or GitHub in System Settings before creating or starting agent
                                        tasks.
                                    </p>
                                </div>
                            </div>
                        </div>
                    )}

                    <div className="rounded-lg border border-border bg-muted/50 p-4 space-y-3">
                        <h3 className="text-sm font-bold text-card-foreground">Execution Options</h3>
                        <div className="flex flex-nowrap gap-3 overflow-x-auto pb-1">
                            <button
                                type="button"
                                role="switch"
                                aria-checked={autoStart}
                                onClick={() => {
                                    if (!repositoryReadOnly) {
                                        setAutoStart((prev) => !prev);
                                    }
                                }}
                                disabled={repositoryReadOnly}
                                className={`${toggleCardClass(autoStart)} ${repositoryReadOnly ? 'cursor-not-allowed opacity-60' : ''}`}
                            >
                                <div className="flex items-center justify-between gap-2 mb-2">
                                    <p className={`text-sm font-medium ${toggleTitleClass(autoStart)}`}>Auto start</p>
                                    <span className={toggleTrackClass(autoStart)}>
                                        <span className={toggleThumbClass(autoStart)} />
                                    </span>
                                </div>
                                <p className="text-xs text-muted-foreground">
                                    {repositoryReadOnly
                                        ? 'Disabled because this project is read-only for coding attempts.'
                                        : 'Create and run attempt right away.'}
                                </p>
                            </button>

                            <button
                                type="button"
                                role="switch"
                                aria-checked={requireReview}
                                onClick={() => {
                                    setRequireReviewTouched(true);
                                    setRequireReview((prev) => !prev);
                                }}
                                className={toggleCardClass(requireReview)}
                            >
                                <div className="flex items-center justify-between gap-2 mb-2">
                                    <p className={`text-sm font-medium ${toggleTitleClass(requireReview)}`}>
                                        Review first
                                    </p>
                                    <span className={toggleTrackClass(requireReview)}>
                                        <span className={toggleThumbClass(requireReview)} />
                                    </span>
                                </div>
                                <p className="text-xs text-muted-foreground">Require manual review before commit.</p>
                            </button>

                            <button
                                type="button"
                                role="switch"
                                aria-checked={autoDeploy}
                                disabled={taskPreviewLocked}
                                onClick={() => {
                                    if (!taskPreviewLocked) {
                                        setAutoDeploy((prev) => !prev);
                                    }
                                }}
                                className={`${toggleCardClass(autoDeploy)} ${taskPreviewLocked ? 'cursor-not-allowed opacity-60' : ''}`}
                            >
                                <div className="flex items-center justify-between gap-2 mb-2">
                                    <p className={`text-sm font-medium ${toggleTitleClass(autoDeploy)}`}>
                                        Task preview
                                    </p>
                                    <span className={toggleTrackClass(autoDeploy)}>
                                        <span className={toggleThumbClass(autoDeploy)} />
                                    </span>
                                </div>
                                <p className="text-xs text-muted-foreground">
                                    {projectSettingsLoading
                                        ? 'Loading project setting...'
                                        : projectTaskPreviewEnabled
                                          ? 'Enabled from Project Settings. Turn it off here to skip preview for this task.'
                                          : 'Disabled because Task Preview is off in Project Settings.'}
                                </p>
                            </button>
                        </div>
                    </div>

                    {membersLoading && <p className="text-xs text-muted-foreground">Loading project members...</p>}

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
                        onClick={handleCreate}
                        disabled={!isFormValid || isCreating}
                        className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        {isCreating ? (
                            <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></span>
                        ) : (
                            <span className="material-symbols-outlined text-[18px]">add_task</span>
                        )}
                        {isCreating ? 'Creating...' : 'Create Task'}
                    </button>
                </div>
            </div>
            <SourceControlSetupRequiredDialog
                isOpen={showSetupDialog}
                onClose={() => setShowSetupDialog(false)}
                contextLabel="Creating a task"
            />
        </div>
    );
}
