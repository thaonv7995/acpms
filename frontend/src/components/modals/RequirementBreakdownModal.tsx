import { useEffect, useMemo, useRef, useState } from 'react';
import type {
    ManualBreakdownTaskDraft,
    Requirement,
    RequirementBreakdownSprintAssignmentMode,
} from '@/api/requirements';
import { confirmRequirementBreakdownManual } from '@/api/requirements';
import { createTask, getTaskAttachmentUploadUrl } from '@/api/tasks';
import {
    cancelAttempt,
    createTaskAttempt,
    getAttempt,
    getAttemptLogs,
    type AgentLog,
    type TaskAttempt,
} from '@/api/taskAttempts';
import type { SprintDto } from '@/api/generated/models';
import type { ProjectMember } from '@/api/projects';
import { TimelineLogDisplay } from '@/components/timeline-log';
import type { TaskType } from '@/shared/types';
import { logger } from '@/lib/logger';

type TaskPriorityValue = 'low' | 'medium' | 'high' | 'critical';

interface RequirementBreakdownModalProps {
    isOpen: boolean;
    onClose: () => void;
    projectId: string;
    requirement: Requirement | null;
    sprints: SprintDto[];
    members?: ProjectMember[];
    onCreated?: () => void;
}

interface ReferenceFileMeta {
    key: string;
    filename: string;
    content_type: string;
    size: number;
    uploaded_at: string;
}

interface ManualTaskDraftState {
    id: string;
    title: string;
    description: string;
    taskType: TaskType;
    priority: TaskPriorityValue;
    assignee: string;
    kind: string;
    references: ReferenceFileMeta[];
    source: 'manual' | 'ai';
}

const TASK_TYPE_OPTIONS: Array<{ value: TaskType; label: string }> = [
    { value: 'feature', label: 'Feature' },
    { value: 'spike', label: 'Spike' },
    { value: 'docs', label: 'Documentation' },
    { value: 'refactor', label: 'Refactor' },
    { value: 'bug', label: 'Bug' },
    { value: 'test', label: 'Test' },
    { value: 'chore', label: 'Chore' },
    { value: 'small_task', label: 'Small Task' },
    { value: 'hotfix', label: 'Hotfix' },
];

const KIND_OPTIONS: Array<{ value: string; label: string }> = [
    { value: 'implementation', label: 'Implementation' },
    { value: 'analysis_session', label: 'Analysis session' },
    { value: 'qa', label: 'QA/Validation' },
    { value: 'documentation', label: 'Documentation' },
];

function normalizeSprintStatus(status?: string | null): string {
    if (!status) return '';
    const lower = status.toLowerCase();
    if (lower === 'planning') return 'planned';
    if (lower === 'completed') return 'closed';
    return lower;
}

function createDraftId(): string {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
        return crypto.randomUUID();
    }
    return `draft-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function createManualDraft(seed?: Partial<ManualTaskDraftState>): ManualTaskDraftState {
    return {
        id: createDraftId(),
        title: seed?.title ?? '',
        description: seed?.description ?? '',
        taskType: seed?.taskType ?? 'feature',
        priority: seed?.priority ?? 'medium',
        assignee: seed?.assignee ?? '',
        kind: seed?.kind ?? 'implementation',
        references: seed?.references ?? [],
        source: seed?.source ?? 'manual',
    };
}

function normalizeTaskType(rawType: unknown): TaskType {
    const value = String(rawType ?? '')
        .trim()
        .toLowerCase()
        .replace(/[\s-]+/g, '_');

    const mapping: Record<string, TaskType> = {
        feature: 'feature',
        bug: 'bug',
        refactor: 'refactor',
        docs: 'docs',
        documentation: 'docs',
        test: 'test',
        hotfix: 'hotfix',
        chore: 'chore',
        spike: 'spike',
        small_task: 'small_task',
        smalltask: 'small_task',
        deploy: 'chore',
        init: 'chore',
    };

    return mapping[value] ?? 'feature';
}

function normalizePriority(rawPriority: unknown): TaskPriorityValue {
    const value = String(rawPriority ?? '')
        .trim()
        .toLowerCase();
    if (value === 'low' || value === 'medium' || value === 'high' || value === 'critical') {
        return value;
    }
    return 'medium';
}

function buildAiBreakdownPrompt(requirement: Requirement): string {
    return `
You are supporting BA/PO/PM to break one requirement into executable tasks.

Requirement title:
${requirement.title}

Requirement detail:
${requirement.content}

STRICT RULES:
1) Analysis only. Do NOT edit files, do NOT run code modifications.
2) Propose 3-12 small tasks.
3) Allowed task_type: feature, bug, refactor, docs, test, chore, hotfix, spike, small_task.
4) Emit progressive lines in this exact format for each task:
BREAKDOWN_TASK {"title":"...","description":"...","task_type":"feature","priority":"medium","kind":"implementation"}
5) Output BREAKDOWN_TASK lines as soon as each task is ready.
`.trim();
}

function extractAiDraftsFromLogContent(content: string): ManualTaskDraftState[] {
    const drafts: ManualTaskDraftState[] = [];
    const lines = content.split(/\r?\n/);

    for (const line of lines) {
        const markerIndex = line.indexOf('BREAKDOWN_TASK');
        if (markerIndex < 0) continue;
        const jsonStart = line.indexOf('{', markerIndex);
        const jsonEnd = line.lastIndexOf('}');
        if (jsonStart < 0 || jsonEnd <= jsonStart) continue;
        try {
            const parsed = JSON.parse(line.slice(jsonStart, jsonEnd + 1)) as Record<string, unknown>;
            const title = String(parsed.title || '').trim();
            if (!title) continue;
            drafts.push(
                createManualDraft({
                    title,
                    description: String(parsed.description || '').trim(),
                    taskType: normalizeTaskType(parsed.task_type),
                    priority: normalizePriority(parsed.priority),
                    kind: String(parsed.kind || 'implementation').trim() || 'implementation',
                    source: 'ai',
                })
            );
        } catch {
            // Ignore malformed lines
        }
    }

    if (drafts.length === 0 && content.includes('"tasks"')) {
        const fencedJsonRegex = /```json([\s\S]*?)```/gi;
        let match: RegExpExecArray | null = fencedJsonRegex.exec(content);
        while (match) {
            try {
                const parsed = JSON.parse(match[1]) as { tasks?: Array<Record<string, unknown>> };
                const tasks = Array.isArray(parsed.tasks) ? parsed.tasks : [];
                for (const task of tasks) {
                    const title = String(task.title || '').trim();
                    if (!title) continue;
                    drafts.push(
                        createManualDraft({
                            title,
                            description: String(task.description || '').trim(),
                            taskType: normalizeTaskType(task.task_type),
                            priority: normalizePriority(task.priority),
                            kind: String(task.kind || 'implementation').trim() || 'implementation',
                            source: 'ai',
                        })
                    );
                }
            } catch {
                // Ignore malformed block
            }
            match = fencedJsonRegex.exec(content);
        }
    }

    return drafts;
}

function isTerminalAttemptStatus(status: string | undefined): boolean {
    const upper = String(status || '').toUpperCase();
    return upper === 'SUCCESS' || upper === 'FAILED' || upper === 'CANCELLED';
}

export function RequirementBreakdownModal({
    isOpen,
    onClose,
    projectId,
    requirement,
    sprints,
    members = [],
    onCreated,
}: RequirementBreakdownModalProps) {
    const [manualTasks, setManualTasks] = useState<ManualTaskDraftState[]>([]);
    const [editingTaskId, setEditingTaskId] = useState<string | null>(null);
    const [recentlyAddedIds, setRecentlyAddedIds] = useState<Set<string>>(new Set());

    const [assignmentMode, setAssignmentMode] =
        useState<RequirementBreakdownSprintAssignmentMode>('backlog');
    const [selectedSprintId, setSelectedSprintId] = useState<string>('');

    const [isCreatingManual, setIsCreatingManual] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const [uploadingTaskId, setUploadingTaskId] = useState<string | null>(null);
    const [fileTargetTaskId, setFileTargetTaskId] = useState<string | null>(null);
    const fileInputRef = useRef<HTMLInputElement | null>(null);

    const [aiPanelOpen, setAiPanelOpen] = useState(false);
    const [isStartingAi, setIsStartingAi] = useState(false);
    const [isCancellingAi, setIsCancellingAi] = useState(false);
    const [aiAttempt, setAiAttempt] = useState<TaskAttempt | null>(null);
    const [aiStatusMessage, setAiStatusMessage] = useState<string | null>(null);

    const seenLogIdsRef = useRef<Set<string>>(new Set());
    const seenTaskSignaturesRef = useRef<Set<string>>(new Set());

    const activeSprint = useMemo(
        () => sprints.find((s) => normalizeSprintStatus(s.status) === 'active') || null,
        [sprints]
    );
    const memberOptions = useMemo(
        () =>
            members.map((member) => ({
                id: member.id,
                name: member.name || member.email,
            })),
        [members]
    );
    const memberNameById = useMemo(
        () => new Map(memberOptions.map((member) => [member.id, member.name])),
        [memberOptions]
    );

    const hasValidSprintAssignment =
        (assignmentMode !== 'selected' || Boolean(selectedSprintId)) &&
        (assignmentMode !== 'active' || Boolean(activeSprint));

    const manualTasksForCreate = useMemo(
        () => manualTasks.filter((task) => task.title.trim().length > 0),
        [manualTasks]
    );

    const canCreateManual = manualTasksForCreate.length > 0 && hasValidSprintAssignment;

    const editingTask = useMemo(
        () => manualTasks.find((task) => task.id === editingTaskId) || null,
        [manualTasks, editingTaskId]
    );

    useEffect(() => {
        if (!isOpen) return;
        setManualTasks([]);
        setEditingTaskId(null);
        setRecentlyAddedIds(new Set());

        setError(null);
        setIsCreatingManual(false);
        setUploadingTaskId(null);
        setFileTargetTaskId(null);

        setAiPanelOpen(false);
        setIsStartingAi(false);
        setIsCancellingAi(false);
        setAiAttempt(null);
        setAiStatusMessage(null);
        seenLogIdsRef.current = new Set();
        seenTaskSignaturesRef.current = new Set();

        if (activeSprint) {
            setAssignmentMode('active');
        } else {
            setAssignmentMode('backlog');
        }
        setSelectedSprintId('');
    }, [isOpen, requirement?.id, activeSprint]);

    useEffect(() => {
        if (members.length === 0) return;
        setManualTasks((prev) =>
            prev.map((task) =>
                task.assignee && !members.some((member) => member.id === task.assignee)
                    ? { ...task, assignee: '' }
                    : task
            )
        );
    }, [members]);

    useEffect(() => {
        if (!aiAttempt?.id) return;
        if (isTerminalAttemptStatus(aiAttempt.status)) return;

        let disposed = false;
        const attemptId = aiAttempt.id;

        const poll = async () => {
            try {
                const [latestAttempt, logs] = await Promise.all([
                    getAttempt(attemptId),
                    getAttemptLogs(attemptId),
                ]);

                if (disposed) return;

                setAiAttempt(latestAttempt);

                const parsedDrafts: ManualTaskDraftState[] = [];
                for (const log of logs) {
                    const logKey = log.id || `${log.created_at}|${log.log_type}|${log.content}`;
                    if (seenLogIdsRef.current.has(logKey)) continue;
                    seenLogIdsRef.current.add(logKey);
                    parsedDrafts.push(...extractAiDraftsFromLogContent(log.content));
                }

                if (parsedDrafts.length > 0) {
                    const accepted = parsedDrafts.filter((draft) => {
                        const signature = [
                            draft.taskType,
                            draft.title.trim().toLowerCase(),
                            draft.description.trim().toLowerCase(),
                        ].join('|');
                        if (seenTaskSignaturesRef.current.has(signature)) return false;
                        seenTaskSignaturesRef.current.add(signature);
                        return true;
                    });

                    if (accepted.length > 0) {
                        const ids = accepted.map((task) => task.id);
                        setManualTasks((prev) => [...prev, ...accepted]);
                        setRecentlyAddedIds((prev) => {
                            const next = new Set(prev);
                            ids.forEach((id) => next.add(id));
                            return next;
                        });
                        window.setTimeout(() => {
                            setRecentlyAddedIds((prev) => {
                                const next = new Set(prev);
                                ids.forEach((id) => next.delete(id));
                                return next;
                            });
                        }, 1800);
                    }
                }

                const statusUpper = latestAttempt.status.toUpperCase();
                if (statusUpper === 'SUCCESS') {
                    setAiStatusMessage('AI analysis completed. New tasks were appended to the list.');
                } else if (statusUpper === 'FAILED') {
                    setAiStatusMessage(
                        latestAttempt.error_message || 'AI analysis failed. Please review logs and retry.'
                    );
                } else if (statusUpper === 'CANCELLED') {
                    setAiStatusMessage('AI analysis was cancelled.');
                } else {
                    setAiStatusMessage(
                        'AI analysis is running. Suggested tasks will append to the list progressively.'
                    );
                }
            } catch (err) {
                if (!disposed) {
                    logger.error('Failed to poll AI breakdown attempt:', err);
                    setError(err instanceof Error ? err.message : 'Failed to refresh AI analysis');
                }
            }
        };

        void poll();
        const timer = window.setInterval(poll, 2000);
        return () => {
            disposed = true;
            window.clearInterval(timer);
        };
    }, [aiAttempt?.id, aiAttempt?.status]);

    if (!isOpen || !requirement) return null;

    const resolveSprintId = (): string | null => {
        if (assignmentMode === 'active') return activeSprint?.id || null;
        if (assignmentMode === 'selected') return selectedSprintId || null;
        return null;
    };

    const addManualTask = () => {
        const next = createManualDraft();
        setManualTasks((prev) => [...prev, next]);
        setEditingTaskId(next.id);
    };

    const removeManualTask = (taskId: string) => {
        setManualTasks((prev) => {
            if (prev.length <= 1) return [];
            return prev.filter((task) => task.id !== taskId);
        });
        if (editingTaskId === taskId) {
            setEditingTaskId(null);
        }
    };

    const updateManualTask = (taskId: string, patch: Partial<ManualTaskDraftState>) => {
        setManualTasks((prev) =>
            prev.map((task) => (task.id === taskId ? { ...task, ...patch } : task))
        );
    };

    const removeReferenceFile = (taskId: string, key: string) => {
        setManualTasks((prev) =>
            prev.map((task) =>
                task.id === taskId
                    ? { ...task, references: task.references.filter((item) => item.key !== key) }
                    : task
            )
        );
    };

    const uploadReferenceFiles = async (taskId: string, files: FileList) => {
        if (files.length === 0) return;
        setUploadingTaskId(taskId);
        setError(null);
        try {
            const uploaded: ReferenceFileMeta[] = [];
            for (const file of Array.from(files)) {
                const contentType = file.type || 'application/octet-stream';
                const presigned = await getTaskAttachmentUploadUrl({
                    project_id: projectId,
                    filename: file.name,
                    content_type: contentType,
                });
                const uploadResponse = await fetch(presigned.upload_url, {
                    method: 'PUT',
                    headers: {
                        'Content-Type': contentType,
                    },
                    body: file,
                });
                if (!uploadResponse.ok) {
                    throw new Error(`Failed to upload ${file.name}`);
                }
                uploaded.push({
                    key: presigned.key,
                    filename: file.name,
                    content_type: contentType,
                    size: file.size,
                    uploaded_at: new Date().toISOString(),
                });
            }

            if (uploaded.length > 0) {
                setManualTasks((prev) =>
                    prev.map((task) =>
                        task.id === taskId
                            ? { ...task, references: [...task.references, ...uploaded] }
                            : task
                    )
                );
            }
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to upload reference files');
        } finally {
            setUploadingTaskId(null);
        }
    };

    const handleOpenFilePicker = (taskId: string) => {
        setFileTargetTaskId(taskId);
        fileInputRef.current?.click();
    };

    const handleFileInputChanged = async (event: React.ChangeEvent<HTMLInputElement>) => {
        const files = event.target.files;
        const targetTaskId = fileTargetTaskId;
        event.currentTarget.value = '';
        if (!files || files.length === 0 || !targetTaskId) return;
        await uploadReferenceFiles(targetTaskId, files);
    };

    const handleCreateManual = async () => {
        if (!canCreateManual) return;
        setIsCreatingManual(true);
        setError(null);
        try {
            const payloadTasks: ManualBreakdownTaskDraft[] = manualTasksForCreate.map((task) => {
                const metadata: Record<string, unknown> = {
                    breakdown_source: task.source,
                    priority: task.priority,
                };
                if (task.references.length > 0) {
                    metadata.reference_files = task.references;
                }

                return {
                    title: task.title.trim(),
                    description: task.description.trim() || undefined,
                    task_type: task.taskType,
                    priority: task.priority,
                    assigned_to: task.assignee || null,
                    kind: task.kind.trim() || undefined,
                    metadata,
                };
            });

            await confirmRequirementBreakdownManual(projectId, requirement.id, {
                assignment_mode: assignmentMode,
                sprint_id: resolveSprintId(),
                tasks: payloadTasks,
            });

            onCreated?.();
            onClose();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to create tasks');
        } finally {
            setIsCreatingManual(false);
        }
    };

    const handleStartAiSupport = async () => {
        if (aiAttempt && !isTerminalAttemptStatus(aiAttempt.status)) {
            setAiStatusMessage('AI analysis is already running.');
            return;
        }

        setAiPanelOpen(true);
        setError(null);
        setAiStatusMessage(
            'AI support can take time. After starting, logs will stream and tasks will append directly into the list.'
        );
        setIsStartingAi(true);

        try {
            const analysisTask = await createTask({
                project_id: projectId,
                requirement_id: requirement.id,
                title: `[Breakdown][AI] ${requirement.title}`,
                description: buildAiBreakdownPrompt(requirement),
                task_type: 'spike',
                metadata: {
                    breakdown_mode: 'ai_support',
                    breakdown_kind: 'analysis_session',
                    requirement_id: requirement.id,
                    no_code_changes: true,
                },
            });

            const attempt = await createTaskAttempt(analysisTask.id);
            setAiAttempt(attempt);
            seenLogIdsRef.current = new Set();
            seenTaskSignaturesRef.current = new Set();
            setAiStatusMessage(
                'AI analysis started. Watch logs on the right panel while tasks append on the left.'
            );
        } catch (err) {
            const message = err instanceof Error ? err.message : 'Failed to start AI support';
            setError(message);
            setAiStatusMessage(message);
        } finally {
            setIsStartingAi(false);
        }
    };

    const handleCancelAiSupport = async () => {
        if (!aiAttempt?.id) return;
        setIsCancellingAi(true);
        setError(null);
        try {
            await cancelAttempt(aiAttempt.id);
            setAiStatusMessage('Cancellation requested for AI analysis.');
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to cancel AI analysis');
        } finally {
            setIsCancellingAi(false);
        }
    };

    const aiTaskCount = manualTasks.filter((task) => task.source === 'ai').length;
    const isAiAttemptActive = Boolean(aiAttempt && !isTerminalAttemptStatus(aiAttempt.status));

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose} />
            <div
                className={`relative w-full max-h-[92vh] overflow-hidden rounded-2xl border border-border bg-card shadow-2xl ${
                    aiPanelOpen ? 'max-w-[96vw]' : 'max-w-5xl'
                }`}
            >
                <div className="px-6 py-4 border-b border-border bg-muted/40 flex items-center justify-between">
                    <div>
                        <h2 className="text-lg font-bold text-card-foreground">Break Requirement into Tasks</h2>
                        <p className="text-xs text-muted-foreground mt-1">
                            Focused task list flow. Add tasks, edit details, then confirm once.
                        </p>
                    </div>
                    <button onClick={onClose} className="text-muted-foreground hover:text-card-foreground">
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                <input
                    ref={fileInputRef}
                    type="file"
                    multiple
                    className="hidden"
                    onChange={handleFileInputChanged}
                />

                <div className="p-6 overflow-y-auto max-h-[calc(92vh-150px)]">
                    <div className={aiPanelOpen ? 'grid grid-cols-1 xl:grid-cols-[1.7fr_1fr] gap-4' : ''}>
                        <div className="space-y-4">
                            <div className="rounded-xl border border-border bg-muted/20 p-4">
                                <p className="text-sm text-muted-foreground">
                                    Requirement: <span className="font-semibold text-card-foreground">{requirement.title}</span>
                                </p>
                            </div>

                            {error && (
                                <div className="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-400">
                                    {error}
                                </div>
                            )}

                            <div className="rounded-xl border border-border bg-muted/20 p-4">
                                <div className="flex items-center justify-between gap-2 mb-3">
                                    <h3 className="text-sm font-semibold text-card-foreground">
                                        Task List ({manualTasksForCreate.length} ready)
                                    </h3>
                                    <div className="flex items-center gap-2">
                                        <button
                                            onClick={() => setAiPanelOpen((prev) => !prev)}
                                            className={`px-2.5 py-1.5 text-xs rounded-lg border border-border hover:bg-muted flex items-center gap-1.5 ${
                                                aiPanelOpen ? 'bg-primary/10 text-primary' : ''
                                            }`}
                                            title="Toggle AI support panel"
                                        >
                                            <span className="material-symbols-outlined text-[14px]">smart_toy</span>
                                            AI
                                        </button>
                                        <button
                                            onClick={addManualTask}
                                            className="px-2.5 py-1.5 text-xs rounded-lg border border-border hover:bg-muted"
                                        >
                                            + Add task
                                        </button>
                                    </div>
                                </div>

                                <div className="space-y-2">
                                    {manualTasks.length === 0 && (
                                        <div className="rounded-lg border border-dashed border-border bg-card/40 px-3 py-4 text-xs text-muted-foreground">
                                            No tasks yet. Click <span className="font-semibold text-card-foreground">+ Add task</span>{' '}
                                            for manual mode, or use <span className="font-semibold text-card-foreground">AI</span> then
                                            <span className="font-semibold text-card-foreground"> Start</span> to append task drafts.
                                        </div>
                                    )}

                                    {manualTasks.map((task, index) => {
                                        const isRecentlyAdded = recentlyAddedIds.has(task.id);
                                        return (
                                            <div
                                                key={task.id}
                                                className={`rounded-lg border border-border bg-card px-3 py-2 transition-colors ${
                                                    isRecentlyAdded ? 'ring-1 ring-primary/60 bg-primary/5' : ''
                                                }`}
                                            >
                                                <div className="flex items-start justify-between gap-3">
                                                    <button
                                                        onClick={() => setEditingTaskId(task.id)}
                                                        className="text-left flex-1 min-w-0"
                                                    >
                                                        <div className="flex items-center gap-2">
                                                            <p className="text-sm font-semibold text-card-foreground truncate">
                                                                {task.title.trim() || `Task ${index + 1} (untitled)`}
                                                            </p>
                                                            {task.source === 'ai' && (
                                                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-primary/15 text-primary">
                                                                    AI
                                                                </span>
                                                            )}
                                                        </div>
                                                        <p className="text-xs text-muted-foreground mt-1">
                                                            {task.taskType} · Priority:{' '}
                                                            <span className="capitalize">{task.priority}</span> · Assignee:{' '}
                                                            {task.assignee
                                                                ? memberNameById.get(task.assignee) || 'Unknown member'
                                                                : 'Unassigned'}{' '}
                                                            · References: {task.references.length}
                                                        </p>
                                                        {task.description.trim() && (
                                                            <p className="text-xs text-muted-foreground/80 mt-1 line-clamp-1">
                                                                {task.description}
                                                            </p>
                                                        )}
                                                    </button>
                                                    <div className="flex items-center gap-1 shrink-0">
                                                        <button
                                                            onClick={() => setEditingTaskId(task.id)}
                                                            className="text-xs px-2 py-1 rounded border border-border hover:bg-muted"
                                                        >
                                                            Edit
                                                        </button>
                                                        <button
                                                            onClick={() => removeManualTask(task.id)}
                                                            className="text-xs px-2 py-1 rounded border border-border hover:bg-muted"
                                                        >
                                                            Remove
                                                        </button>
                                                    </div>
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>

                            {editingTask && (
                                <div className="rounded-xl border border-border bg-card p-4 space-y-3">
                                    <div className="flex items-center justify-between">
                                        <h3 className="text-sm font-semibold text-card-foreground">Task Editor</h3>
                                        <button
                                            onClick={() => setEditingTaskId(null)}
                                            className="text-xs px-2 py-1 rounded border border-border hover:bg-muted"
                                        >
                                            Done
                                        </button>
                                    </div>

                                    <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
                                        <input
                                            value={editingTask.title}
                                            onChange={(e) =>
                                                updateManualTask(editingTask.id, { title: e.target.value })
                                            }
                                            placeholder="Task title"
                                            className="md:col-span-2 rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                        />
                                        <select
                                            value={editingTask.taskType}
                                            onChange={(e) =>
                                                updateManualTask(editingTask.id, {
                                                    taskType: normalizeTaskType(e.target.value),
                                                })
                                            }
                                            className="rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                        >
                                            {TASK_TYPE_OPTIONS.map((option) => (
                                                <option key={option.value} value={option.value}>
                                                    {option.label}
                                                </option>
                                            ))}
                                        </select>
                                        <select
                                            value={editingTask.priority}
                                            onChange={(e) =>
                                                updateManualTask(editingTask.id, {
                                                    priority: normalizePriority(e.target.value),
                                                })
                                            }
                                            className="rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                        >
                                            <option value="low">Low</option>
                                            <option value="medium">Medium</option>
                                            <option value="high">High</option>
                                            <option value="critical">Critical</option>
                                        </select>
                                    </div>

                                    <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                                        <select
                                            value={editingTask.kind}
                                            onChange={(e) =>
                                                updateManualTask(editingTask.id, { kind: e.target.value })
                                            }
                                            className="rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                        >
                                            {KIND_OPTIONS.map((option) => (
                                                <option key={option.value} value={option.value}>
                                                    {option.label}
                                                </option>
                                            ))}
                                        </select>
                                        <select
                                            value={editingTask.assignee}
                                            onChange={(e) =>
                                                updateManualTask(editingTask.id, { assignee: e.target.value })
                                            }
                                            className="rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                        >
                                            <option value="">Unassigned</option>
                                            {memberOptions.map((member) => (
                                                <option key={member.id} value={member.id}>
                                                    {member.name}
                                                </option>
                                            ))}
                                        </select>
                                    </div>

                                    <textarea
                                        value={editingTask.description}
                                        onChange={(e) =>
                                            updateManualTask(editingTask.id, { description: e.target.value })
                                        }
                                        rows={4}
                                        placeholder="Description / expected output"
                                        className="w-full rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                    />

                                    <div className="rounded-lg border border-border bg-muted/20 p-3">
                                        <div className="flex items-center justify-between gap-2 mb-2">
                                            <h4 className="text-xs font-semibold text-card-foreground">
                                                Reference Files ({editingTask.references.length})
                                            </h4>
                                            <button
                                                onClick={() => handleOpenFilePicker(editingTask.id)}
                                                disabled={uploadingTaskId === editingTask.id}
                                                className="text-xs px-2 py-1 rounded border border-border hover:bg-muted disabled:opacity-60"
                                            >
                                                {uploadingTaskId === editingTask.id ? 'Uploading...' : 'Upload files'}
                                            </button>
                                        </div>
                                        {editingTask.references.length === 0 ? (
                                            <p className="text-xs text-muted-foreground">
                                                No reference files yet.
                                            </p>
                                        ) : (
                                            <div className="space-y-1.5">
                                                {editingTask.references.map((file) => (
                                                    <div
                                                        key={file.key}
                                                        className="flex items-center justify-between gap-2 text-xs rounded border border-border bg-card px-2 py-1.5"
                                                    >
                                                        <div className="min-w-0">
                                                            <p className="text-card-foreground truncate">{file.filename}</p>
                                                            <p className="text-muted-foreground">
                                                                {Math.round(file.size / 1024)} KB
                                                            </p>
                                                        </div>
                                                        <button
                                                            onClick={() => removeReferenceFile(editingTask.id, file.key)}
                                                            className="text-muted-foreground hover:text-card-foreground"
                                                        >
                                                            <span className="material-symbols-outlined text-[14px]">close</span>
                                                        </button>
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}

                            <div className="rounded-xl border border-border bg-muted/30 p-4">
                                <h3 className="text-sm font-semibold text-card-foreground mb-3">Sprint Assignment</h3>
                                <div className="space-y-3">
                                    <label className="flex items-start gap-2 text-sm">
                                        <input
                                            type="radio"
                                            name="sprint-assignment"
                                            className="mt-1"
                                            checked={assignmentMode === 'active'}
                                            disabled={!activeSprint}
                                            onChange={() => setAssignmentMode('active')}
                                        />
                                        <span className="text-muted-foreground">
                                            Assign all tasks to active sprint
                                            {activeSprint ? (
                                                <span className="text-card-foreground"> ({activeSprint.name})</span>
                                            ) : (
                                                <span> (no active sprint)</span>
                                            )}
                                        </span>
                                    </label>
                                    <label className="flex items-start gap-2 text-sm">
                                        <input
                                            type="radio"
                                            name="sprint-assignment"
                                            className="mt-1"
                                            checked={assignmentMode === 'selected'}
                                            onChange={() => setAssignmentMode('selected')}
                                        />
                                        <span className="text-muted-foreground">Assign all tasks to selected sprint</span>
                                    </label>
                                    {assignmentMode === 'selected' && (
                                        <select
                                            value={selectedSprintId}
                                            onChange={(e) => setSelectedSprintId(e.target.value)}
                                            className="w-full rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                        >
                                            <option value="">Select sprint...</option>
                                            {sprints.map((sprint) => (
                                                <option key={sprint.id} value={sprint.id}>
                                                    {sprint.name}
                                                </option>
                                            ))}
                                        </select>
                                    )}
                                    <label className="flex items-start gap-2 text-sm">
                                        <input
                                            type="radio"
                                            name="sprint-assignment"
                                            className="mt-1"
                                            checked={assignmentMode === 'backlog'}
                                            onChange={() => setAssignmentMode('backlog')}
                                        />
                                        <span className="text-muted-foreground">Keep tasks in backlog (no sprint)</span>
                                    </label>
                                </div>
                            </div>
                        </div>

                        {aiPanelOpen && (
                            <aside className="rounded-xl border border-border bg-card flex flex-col min-h-[560px] max-h-[calc(92vh-210px)]">
                                <div className="px-4 py-3 border-b border-border flex items-center justify-between">
                                    <div>
                                        <h3 className="text-sm font-semibold text-card-foreground">AI Support Logs</h3>
                                        <p className="text-[11px] text-muted-foreground">
                                            AI may take time. Watch stream and appended tasks.
                                        </p>
                                    </div>
                                    <button
                                        onClick={() => setAiPanelOpen(false)}
                                        className="text-muted-foreground hover:text-card-foreground"
                                    >
                                        <span className="material-symbols-outlined text-[18px]">close</span>
                                    </button>
                                </div>

                                <div className="px-4 py-3 border-b border-border flex items-center gap-2">
                                    <button
                                        onClick={handleStartAiSupport}
                                        disabled={isStartingAi || isAiAttemptActive}
                                        className="px-2.5 py-1.5 text-xs rounded-lg border border-border hover:bg-muted disabled:opacity-60 flex items-center gap-1.5"
                                    >
                                        {isStartingAi ? (
                                            <>
                                                <span className="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                                                Starting
                                            </>
                                        ) : isAiAttemptActive ? (
                                            <>
                                                <span className="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                                                Running
                                            </>
                                        ) : (
                                            <>
                                                <span className="material-symbols-outlined text-[14px]">play_arrow</span>
                                                Start
                                            </>
                                        )}
                                    </button>
                                    <button
                                        onClick={handleCancelAiSupport}
                                        disabled={!aiAttempt || isCancellingAi}
                                        className="px-2.5 py-1.5 text-xs rounded-lg border border-border hover:bg-muted disabled:opacity-60"
                                    >
                                        {isCancellingAi ? 'Cancelling...' : 'Cancel'}
                                    </button>
                                </div>

                                <div className="px-4 py-2 border-b border-border text-xs text-muted-foreground">
                                    <p>AI tasks appended: {aiTaskCount}</p>
                                    {aiAttempt && <p>Status: {aiAttempt.status}</p>}
                                    {aiStatusMessage && <p className="mt-1">{aiStatusMessage}</p>}
                                </div>

                                <div className="flex-1 overflow-y-auto p-3 bg-black/20 font-mono text-[11px] leading-relaxed space-y-2">
                                    {!aiAttempt ? (
                                        <p className="text-muted-foreground">
                                            No logs yet. Click Start to spawn AI analysis attempt.
                                        </p>
                                    ) : (
                                        <div className="h-full min-h-[420px]">
                                            <TimelineLogDisplay
                                                attemptId={aiAttempt.id}
                                                attemptStatus={aiAttempt.status}
                                                showStatusInHeader
                                                showTokenUsageInHeader={false}
                                            />
                                        </div>
                                    )}
                                </div>
                            </aside>
                        )}
                    </div>
                </div>

                <div className="px-6 py-4 border-t border-border bg-muted/30 flex items-center justify-between gap-3">
                    <div className="text-xs text-muted-foreground">
                        {manualTasksForCreate.length} task(s) ready for confirm
                    </div>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={onClose}
                            className="px-3 py-2 text-sm border border-border rounded-lg hover:bg-muted"
                        >
                            Close
                        </button>
                        <button
                            onClick={handleCreateManual}
                            disabled={!canCreateManual || isCreatingManual}
                            className="px-3 py-2 text-sm bg-primary text-primary-foreground rounded-lg font-semibold hover:bg-primary/90 disabled:opacity-60"
                        >
                            {isCreatingManual ? 'Creating Tasks...' : 'Confirm & Create Tasks'}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
