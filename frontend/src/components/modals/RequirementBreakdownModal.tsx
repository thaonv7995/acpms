import { useEffect, useMemo, useRef, useState } from 'react';
import type { Requirement } from '@/api/requirements';
import type { SprintDto } from '@/api/generated/models';
import {
    confirmRequirementBreakdownManual,
    type ManualBreakdownTaskDraft,
    type RequirementBreakdownSprintAssignmentMode,
} from '@/api/requirements';
import { createTask, type Task } from '@/api/tasks';
import {
    cancelAttempt,
    createTaskAttempt,
    getAttempt,
    getAttemptLogs,
    type TaskAttempt,
} from '@/api/taskAttempts';
import type { TaskType } from '@/shared/types';
import type { KanbanTask } from '@/types/project';
import { ViewLogsModal } from './ViewLogsModal';
import { logger } from '@/lib/logger';

interface RequirementBreakdownModalProps {
    isOpen: boolean;
    onClose: () => void;
    projectId: string;
    requirement: Requirement | null;
    sprints: SprintDto[];
    onCreated?: () => void;
}

interface ManualTaskDraft {
    id: string;
    title: string;
    description: string;
    taskType: TaskType;
    estimate: string;
    kind: string;
    source: 'manual' | 'ai';
}

const MANUAL_TASK_TYPE_OPTIONS: Array<{ value: TaskType; label: string }> = [
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

function createManualDraft(seed?: Partial<ManualTaskDraft>): ManualTaskDraft {
    return {
        id: createDraftId(),
        title: seed?.title ?? '',
        description: seed?.description ?? '',
        taskType: seed?.taskType ?? 'feature',
        estimate: seed?.estimate ?? '',
        kind: seed?.kind ?? 'implementation',
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

function normalizeTaskStatus(rawStatus: string | undefined): KanbanTask['status'] {
    const status = String(rawStatus ?? 'todo')
        .trim()
        .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
        .replace(/[\s-]+/g, '_')
        .toLowerCase();
    if (status === 'in_progress') return 'in_progress';
    if (status === 'in_review') return 'in_review';
    if (status === 'done' || status === 'archived' || status === 'cancelled' || status === 'canceled') {
        return 'done';
    }
    return 'todo';
}

function toKanbanTaskType(taskType: TaskType): KanbanTask['type'] {
    if (taskType === 'init') return 'chore';
    return taskType;
}

function toKanbanTask(task: Task, projectId: string): KanbanTask {
    const normalizedType = normalizeTaskType(task.task_type);
    return {
        id: task.id,
        title: task.title,
        description: task.description,
        type: toKanbanTaskType(normalizedType),
        status: normalizeTaskStatus(task.status),
        priority: (task.metadata?.priority as KanbanTask['priority']) || 'medium',
        metadata: task.metadata,
        projectId,
        createdAt: task.created_at,
    };
}

function buildAiBreakdownPrompt(requirement: Requirement): string {
    return `
You are helping BA/PO/PM break a requirement into execution tasks.

Requirement title:
${requirement.title}

Requirement content:
${requirement.content}

Rules:
1. Analysis only. Do not edit files, do not run code generators, do not open PR/MR.
2. Propose 3-12 tasks, each task must be small and actionable.
3. Allowed task types: feature, bug, refactor, docs, test, chore, hotfix, spike, small_task.
4. For each proposed task, output ONE line exactly in this format:
BREAKDOWN_TASK {"title":"...","description":"...","task_type":"feature","estimate":"S","kind":"implementation"}
5. Emit BREAKDOWN_TASK lines progressively as you reason, not only at the end.
6. After finishing, output one final JSON object with key "tasks".
`.trim();
}

function extractAiDraftsFromLogContent(content: string): ManualTaskDraft[] {
    const drafts: ManualTaskDraft[] = [];
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
                    estimate: String(parsed.estimate || '').trim(),
                    kind: String(parsed.kind || 'implementation').trim() || 'implementation',
                    source: 'ai',
                })
            );
        } catch {
            // Ignore malformed lines; next logs may contain valid task drafts.
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
                            estimate: String(task.estimate || '').trim(),
                            kind: String(task.kind || 'implementation').trim() || 'implementation',
                            source: 'ai',
                        })
                    );
                }
            } catch {
                // Ignore malformed JSON block; continue scanning.
            }
            match = fencedJsonRegex.exec(content);
        }
    }

    return drafts;
}

export function RequirementBreakdownModal({
    isOpen,
    onClose,
    projectId,
    requirement,
    sprints,
    onCreated,
}: RequirementBreakdownModalProps) {
    const [manualTasks, setManualTasks] = useState<ManualTaskDraft[]>([createManualDraft()]);
    const [recentlyAddedIds, setRecentlyAddedIds] = useState<Set<string>>(new Set());

    const [assignmentMode, setAssignmentMode] =
        useState<RequirementBreakdownSprintAssignmentMode>('backlog');
    const [selectedSprintId, setSelectedSprintId] = useState<string>('');

    const [isCreatingManual, setIsCreatingManual] = useState(false);
    const [isStartingAi, setIsStartingAi] = useState(false);
    const [isCancellingAi, setIsCancellingAi] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const [aiTask, setAiTask] = useState<Task | null>(null);
    const [aiAttempt, setAiAttempt] = useState<TaskAttempt | null>(null);
    const [showAiLogs, setShowAiLogs] = useState(false);
    const [aiStatusMessage, setAiStatusMessage] = useState<string | null>(null);

    const seenLogIdsRef = useRef<Set<string>>(new Set());
    const seenDraftSignaturesRef = useRef<Set<string>>(new Set());

    const activeSprint = useMemo(
        () => sprints.find((s) => normalizeSprintStatus(s.status) === 'active') || null,
        [sprints]
    );

    const hasValidSprintAssignment =
        (assignmentMode !== 'selected' || Boolean(selectedSprintId)) &&
        (assignmentMode !== 'active' || Boolean(activeSprint));

    const manualTasksForCreate = useMemo(
        () => manualTasks.filter((task) => task.title.trim().length > 0),
        [manualTasks]
    );

    const canCreateManual = manualTasksForCreate.length > 0 && hasValidSprintAssignment;

    useEffect(() => {
        if (!isOpen) return;
        setManualTasks([createManualDraft()]);
        setRecentlyAddedIds(new Set());
        setError(null);
        setIsCreatingManual(false);
        setIsStartingAi(false);
        setIsCancellingAi(false);
        setAiTask(null);
        setAiAttempt(null);
        setShowAiLogs(false);
        setAiStatusMessage(null);
        seenLogIdsRef.current = new Set();
        seenDraftSignaturesRef.current = new Set();

        if (activeSprint) {
            setAssignmentMode('active');
        } else {
            setAssignmentMode('backlog');
        }
        setSelectedSprintId('');
    }, [isOpen, requirement?.id, activeSprint]);

    useEffect(() => {
        if (!aiAttempt?.id) return;
        const statusUpper = aiAttempt.status.toUpperCase();
        if (['SUCCESS', 'FAILED', 'CANCELLED'].includes(statusUpper)) return;

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

                const newDrafts: ManualTaskDraft[] = [];
                for (const log of logs) {
                    const logKey = log.id || `${log.created_at}|${log.log_type}|${log.content}`;
                    if (seenLogIdsRef.current.has(logKey)) continue;
                    seenLogIdsRef.current.add(logKey);
                    newDrafts.push(...extractAiDraftsFromLogContent(log.content));
                }

                if (newDrafts.length > 0) {
                    const acceptedDrafts = newDrafts.filter((draft) => {
                        const signature = [
                            draft.taskType,
                            draft.title.trim().toLowerCase(),
                            draft.description.trim().toLowerCase(),
                        ].join('|');
                        if (seenDraftSignaturesRef.current.has(signature)) return false;
                        seenDraftSignaturesRef.current.add(signature);
                        return true;
                    });

                    if (acceptedDrafts.length > 0) {
                        const acceptedIds = acceptedDrafts.map((task) => task.id);
                        setManualTasks((prev) => {
                            const hasOnlyBlankDraft =
                                prev.length === 1 &&
                                prev[0].title.trim() === '' &&
                                prev[0].description.trim() === '' &&
                                prev[0].source === 'manual';
                            const base = hasOnlyBlankDraft ? [] : prev;
                            return [...base, ...acceptedDrafts];
                        });
                        setRecentlyAddedIds((prev) => {
                            const next = new Set(prev);
                            acceptedIds.forEach((id) => next.add(id));
                            return next;
                        });
                        window.setTimeout(() => {
                            setRecentlyAddedIds((prev) => {
                                const next = new Set(prev);
                                acceptedIds.forEach((id) => next.delete(id));
                                return next;
                            });
                        }, 1800);
                    }
                }

                const normalizedStatus = latestAttempt.status.toUpperCase();
                if (normalizedStatus === 'SUCCESS') {
                    setAiStatusMessage(
                        'AI analysis completed. Review appended task drafts before confirm.'
                    );
                } else if (normalizedStatus === 'FAILED') {
                    setAiStatusMessage(
                        latestAttempt.error_message || 'AI analysis failed. You can retry.'
                    );
                } else if (normalizedStatus === 'CANCELLED') {
                    setAiStatusMessage('AI analysis was cancelled.');
                } else {
                    setAiStatusMessage(
                        'AI analysis is running. Suggested tasks will be appended to the list as they are generated.'
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
        setManualTasks((prev) => [...prev, createManualDraft()]);
    };

    const removeManualTask = (id: string) => {
        setManualTasks((prev) => {
            if (prev.length <= 1) return prev;
            return prev.filter((task) => task.id !== id);
        });
    };

    const updateManualTask = (id: string, patch: Partial<ManualTaskDraft>) => {
        setManualTasks((prev) =>
            prev.map((task) => (task.id === id ? { ...task, ...patch } : task))
        );
    };

    const handleCreateManual = async () => {
        if (!canCreateManual) return;

        setIsCreatingManual(true);
        setError(null);
        try {
            const payloadTasks: ManualBreakdownTaskDraft[] = manualTasksForCreate.map((task) => ({
                title: task.title.trim(),
                description: task.description.trim() || undefined,
                task_type: task.taskType,
                estimate: task.estimate.trim() || undefined,
                kind: task.kind.trim() || undefined,
            }));

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
        setError(null);
        setAiStatusMessage(
            'AI support can take a while. System will spawn one analysis agent and stream logs.'
        );
        setIsStartingAi(true);

        try {
            const task = await createTask({
                project_id: projectId,
                requirement_id: requirement.id,
                sprint_id: undefined,
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

            const attempt = await createTaskAttempt(task.id);

            setAiTask(task);
            setAiAttempt(attempt);
            setShowAiLogs(true);
            setAiStatusMessage(
                'AI analysis started. Attempt logs opened; task drafts will append here progressively.'
            );
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to start AI support analysis');
        } finally {
            setIsStartingAi(false);
        }
    };

    const handleCancelAi = async () => {
        if (!aiAttempt?.id) return;
        setIsCancellingAi(true);
        setError(null);
        try {
            await cancelAttempt(aiAttempt.id);
            setAiStatusMessage('AI analysis cancellation requested.');
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to cancel AI analysis');
        } finally {
            setIsCancellingAi(false);
        }
    };

    return (
        <>
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
                <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose} />
                <div className="relative w-full max-w-4xl max-h-[90vh] overflow-hidden rounded-2xl border border-border bg-card shadow-2xl">
                    <div className="px-6 py-4 border-b border-border bg-muted/40 flex items-center justify-between">
                        <div>
                            <h2 className="text-lg font-bold text-card-foreground">Break Requirement into Tasks</h2>
                            <p className="text-xs text-muted-foreground mt-1">
                                Manual-first. Add/edit task list then confirm in one API call.
                            </p>
                        </div>
                        <button onClick={onClose} className="text-muted-foreground hover:text-card-foreground">
                            <span className="material-symbols-outlined">close</span>
                        </button>
                    </div>

                    <div className="p-6 overflow-y-auto max-h-[calc(90vh-150px)] space-y-4">
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
                            <div className="flex items-center justify-between gap-3 flex-wrap">
                                <div>
                                    <h3 className="text-sm font-semibold text-card-foreground">AI Support (Optional)</h3>
                                    <p className="text-xs text-muted-foreground mt-1">
                                        AI analysis may take time. It will spawn an agent attempt and stream logs.
                                    </p>
                                </div>
                                <div className="flex items-center gap-2">
                                    <button
                                        onClick={handleStartAiSupport}
                                        disabled={isStartingAi}
                                        className="px-3 py-2 text-sm rounded-lg border border-border hover:bg-muted disabled:opacity-60 flex items-center gap-2"
                                    >
                                        {isStartingAi ? (
                                            <>
                                                <span className="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
                                                Starting...
                                            </>
                                        ) : (
                                            <>
                                                <span className="material-symbols-outlined text-[16px]">smart_toy</span>
                                                Run AI Analysis
                                            </>
                                        )}
                                    </button>
                                    <button
                                        onClick={() => setShowAiLogs(true)}
                                        disabled={!aiTask || !aiAttempt}
                                        className="px-3 py-2 text-sm rounded-lg border border-border hover:bg-muted disabled:opacity-60"
                                    >
                                        Open Attempt Logs
                                    </button>
                                    <button
                                        onClick={handleCancelAi}
                                        disabled={!aiAttempt || isCancellingAi}
                                        className="px-3 py-2 text-sm rounded-lg border border-border hover:bg-muted disabled:opacity-60"
                                    >
                                        {isCancellingAi ? 'Cancelling...' : 'Cancel AI'}
                                    </button>
                                </div>
                            </div>
                            {aiAttempt && (
                                <p className="text-xs text-muted-foreground mt-2">
                                    Attempt status: <span className="font-semibold text-card-foreground">{aiAttempt.status}</span>
                                </p>
                            )}
                            {aiStatusMessage && (
                                <p className="text-xs text-muted-foreground mt-2">{aiStatusMessage}</p>
                            )}
                        </div>

                        <div className="space-y-3">
                            <div className="flex items-center justify-between gap-3">
                                <h3 className="text-sm font-semibold text-card-foreground">
                                    Task Draft List ({manualTasksForCreate.length} ready)
                                </h3>
                                <button
                                    onClick={addManualTask}
                                    className="px-3 py-1.5 text-xs rounded-lg border border-border hover:bg-muted"
                                >
                                    + Add task
                                </button>
                            </div>
                            {manualTasks.map((task, index) => {
                                const isRecentlyAdded = recentlyAddedIds.has(task.id);
                                return (
                                    <div
                                        key={task.id}
                                        className={`rounded-xl border border-border bg-card p-4 transition-colors ${
                                            isRecentlyAdded ? 'ring-1 ring-primary/60 bg-primary/5' : ''
                                        }`}
                                    >
                                        <div className="flex items-center justify-between gap-2 mb-3">
                                            <div className="flex items-center gap-2">
                                                <p className="text-sm font-semibold text-card-foreground">Task {index + 1}</p>
                                                {task.source === 'ai' && (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-primary/15 text-primary">
                                                        AI
                                                    </span>
                                                )}
                                            </div>
                                            <button
                                                onClick={() => removeManualTask(task.id)}
                                                disabled={manualTasks.length <= 1}
                                                className="text-xs text-muted-foreground hover:text-card-foreground disabled:opacity-40"
                                            >
                                                Remove
                                            </button>
                                        </div>
                                        <div className="grid grid-cols-1 md:grid-cols-4 gap-3 mb-3">
                                            <input
                                                value={task.title}
                                                onChange={(e) => updateManualTask(task.id, { title: e.target.value })}
                                                placeholder="Task title"
                                                className="md:col-span-2 rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                            />
                                            <select
                                                value={task.taskType}
                                                onChange={(e) =>
                                                    updateManualTask(task.id, {
                                                        taskType: normalizeTaskType(e.target.value),
                                                    })
                                                }
                                                className="rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                            >
                                                {MANUAL_TASK_TYPE_OPTIONS.map((option) => (
                                                    <option key={option.value} value={option.value}>
                                                        {option.label}
                                                    </option>
                                                ))}
                                            </select>
                                            <input
                                                value={task.estimate}
                                                onChange={(e) => updateManualTask(task.id, { estimate: e.target.value })}
                                                placeholder="Estimate (S/M/L)"
                                                className="rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                            />
                                        </div>
                                        <textarea
                                            value={task.description}
                                            onChange={(e) => updateManualTask(task.id, { description: e.target.value })}
                                            rows={3}
                                            placeholder="Description / expected output"
                                            className="w-full rounded-lg border border-border bg-card px-3 py-2 text-sm"
                                        />
                                    </div>
                                );
                            })}
                        </div>

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

                        <div className="rounded-xl border border-border bg-muted/30 p-4">
                            <p className="text-xs text-muted-foreground">
                                Confirm will create all listed tasks in one batch API call. New tasks are created with status{' '}
                                <span className="font-semibold text-card-foreground">todo</span> and no auto-attempt.
                            </p>
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

            {showAiLogs && aiTask && aiAttempt && (
                <ViewLogsModal
                    isOpen={showAiLogs}
                    onClose={() => setShowAiLogs(false)}
                    task={toKanbanTask(aiTask, projectId)}
                    projectId={projectId}
                    initialAttemptId={aiAttempt.id}
                />
            )}
        </>
    );
}
