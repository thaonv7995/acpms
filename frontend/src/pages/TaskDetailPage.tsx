import { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { AppShell } from '../components/layout/AppShell';
import { ConfigureAgentModal, ViewLogsModal, EditTaskModal, ConfirmModal } from '../components/modals';
import { TaskDetailHeader, TaskMetadataSidebar, DiffViewerModal, TaskStatusContent } from '../components/task-detail-page';
import { prefetchDiffData } from '../components/diff-viewer/useDiff';
import { getTask, deleteTask, Task, type TaskStatus } from '../api/tasks';
import { getTaskAttempts, createTaskAttempt, TaskAttempt } from '../api/taskAttempts';
import type { KanbanTask } from '../types/project';
import { logger } from '@/lib/logger';

// Status display helpers
const statusLabels: Record<string, string> = {
    backlog: 'BACKLOG', Backlog: 'BACKLOG',
    todo: 'TO DO', Todo: 'TO DO',
    in_progress: 'IN PROGRESS', InProgress: 'IN PROGRESS',
    in_review: 'IN REVIEW', InReview: 'IN REVIEW',
    blocked: 'BLOCKED', Blocked: 'BLOCKED',
    done: 'DONE', Done: 'DONE',
    archived: 'ARCHIVED', Archived: 'ARCHIVED',
};

const statusColors: Record<string, string> = {
    backlog: 'bg-slate-500', Backlog: 'bg-slate-500',
    todo: 'bg-slate-400', Todo: 'bg-slate-400',
    in_progress: 'bg-blue-500', InProgress: 'bg-blue-500',
    in_review: 'bg-yellow-500', InReview: 'bg-yellow-500',
    blocked: 'bg-red-500', Blocked: 'bg-red-500',
    done: 'bg-green-500', Done: 'bg-green-500',
    archived: 'bg-slate-500', Archived: 'bg-slate-500',
};

function TaskDetailSkeleton() {
    return (
        <div className="animate-pulse flex flex-col gap-6">
            <div className="flex items-center gap-4">
                <div className="h-6 w-6 bg-muted rounded"></div>
                <div className="h-8 w-96 bg-muted rounded"></div>
            </div>
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                <div className="lg:col-span-2 h-96 bg-muted rounded-xl"></div>
                <div className="h-96 bg-muted rounded-xl"></div>
            </div>
        </div>
    );
}

function getLatestSuccessAttempt(attempts: TaskAttempt[]): TaskAttempt | null {
    const sortedAttempts = [...attempts].sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
    );
    return sortedAttempts.find((attempt) => attempt.status.toLowerCase() === 'success') || null;
}

export function TaskDetailPage() {
    const { projectId, taskId } = useParams<{ projectId: string; taskId: string }>();
    const navigate = useNavigate();

    logger.log('[TaskDetailPage] Rendered with projectId:', projectId, 'taskId:', taskId);

    const [task, setTask] = useState<Task | null>(null);
    const [attempts, setAttempts] = useState<TaskAttempt[]>([]);
    const [latestSuccessAttempt, setLatestSuccessAttempt] = useState<TaskAttempt | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [showDiffViewer, setShowDiffViewer] = useState(false);
    const [isPreparingDiffViewer, setIsPreparingDiffViewer] = useState(false);
    const [showAgentConfig, setShowAgentConfig] = useState(false);
    const [showLogsDrawer, setShowLogsDrawer] = useState(false);
    const [showEditTaskModal, setShowEditTaskModal] = useState(false);
    const [showDeleteTaskModal, setShowDeleteTaskModal] = useState(false);
    const [isDeletingTask, setIsDeletingTask] = useState(false);

    const refreshTaskData = useCallback(async (targetTaskId: string) => {
        const [taskData, attemptsData] = await Promise.all([
            getTask(targetTaskId),
            getTaskAttempts(targetTaskId),
        ]);
        setTask(taskData);
        setAttempts(attemptsData);
        setLatestSuccessAttempt(getLatestSuccessAttempt(attemptsData));
    }, []);

    useEffect(() => {
        if (!taskId) return;

        const fetchData = async () => {
            try {
                setLoading(true);
                setError(null);
                await refreshTaskData(taskId);
            } catch (err) {
                logger.error('Failed to fetch task:', err);
                setError('Failed to load task details');
            } finally {
                setLoading(false);
            }
        };

        fetchData();
    }, [taskId, refreshTaskData]);

    const handleStartAgent = useCallback(async () => {
        if (!task) return;
        try {
            const targetTaskId = task.id;
            await createTaskAttempt(targetTaskId);
            await refreshTaskData(targetTaskId);
        } catch (err) {
            logger.error('Failed to start agent:', err);
            throw err instanceof Error
                ? err
                : new Error('Failed to start task execution. Please try again.');
        }
    }, [task, refreshTaskData]);

    const handleBack = () => {
        // Check if we came from project context or task board
        if (projectId) {
            // Came from project page
            navigate(`/projects/${projectId}`);
        } else {
            // Came from task board (/tasks)
            navigate('/tasks');
        }
    };

    const handleReviewChanges = useCallback(async () => {
        if (!latestSuccessAttempt || isPreparingDiffViewer) {
            return;
        }

        setIsPreparingDiffViewer(true);
        try {
            await prefetchDiffData(latestSuccessAttempt.id);
        } catch (err) {
            logger.warn('Prefetch review changes failed', err);
        } finally {
            setShowDiffViewer(true);
            setIsPreparingDiffViewer(false);
        }
    }, [latestSuccessAttempt, isPreparingDiffViewer]);

    const normalizeStatus = (status: string) =>
        status.replace(/([a-z])([A-Z])/g, '$1_$2').toLowerCase();

    const handleDeleteTask = useCallback(async () => {
        if (!task || isDeletingTask) return;

        setIsDeletingTask(true);
        try {
            await deleteTask(task.id);
            setShowDeleteTaskModal(false);
            handleBack();
        } finally {
            setIsDeletingTask(false);
        }
    }, [task, isDeletingTask]);

    if (loading) {
        return (
            <AppShell>
                <div className="flex-1 overflow-y-auto p-6 md:p-8">
                    <div className="max-w-6xl mx-auto"><TaskDetailSkeleton /></div>
                </div>
            </AppShell>
        );
    }

    if (error || !task) {
        return (
            <AppShell>
                <div className="flex-1 overflow-y-auto p-6 md:p-8">
                    <div className="max-w-6xl mx-auto">
                        <div className="bg-red-100 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg">
                            {error || 'Task not found'}
                        </div>
                    </div>
                </div>
            </AppShell>
        );
    }

    const normalizedStatus = normalizeStatus(task.status);
    const isInReview = normalizedStatus === 'in_review';
    const showStartAgent = normalizedStatus !== 'backlog' && normalizedStatus !== 'archived';
    const displayStatus = statusLabels[task.status] || task.status.toUpperCase();
    const statusColor = statusColors[task.status] || 'bg-slate-400';

    return (
        <AppShell>
            <div className="flex-1 overflow-y-auto p-6 md:p-8 scrollbar-hide">
                <div className="max-w-6xl mx-auto flex flex-col gap-6">
                    <TaskDetailHeader
                        taskId={task.id}
                        title={task.title}
                        status={task.status}
                        displayStatus={displayStatus}
                        statusColor={statusColor}
                        isInReview={isInReview}
                        hasReviewableAttempt={!!latestSuccessAttempt}
                        hasAttempts={attempts.length > 0}
                        onBack={handleBack}
                        onReviewChanges={handleReviewChanges}
                        onStartAgent={() => setShowAgentConfig(true)}
                        onViewAttempts={() => setShowLogsDrawer(true)}
                        showStartAgent={showStartAgent}
                    />

                    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                        <div className="lg:col-span-2">
                            <TaskStatusContent
                                task={task}
                                normalizedStatus={normalizedStatus}
                                artifactAttemptId={latestSuccessAttempt?.id}
                                previewMetadata={
                                    (latestSuccessAttempt?.metadata as Record<string, unknown> | undefined) ??
                                    (task.metadata as Record<string, unknown> | undefined)
                                }
                            />
                        </div>

                        <TaskMetadataSidebar
                            taskId={task.id}
                            status={normalizedStatus as TaskStatus}
                            priority={(task.metadata?.priority as string) || 'medium'}
                            type={task.task_type || 'feature'}
                            createdAt={task.created_at}
                            onEditTask={() => setShowEditTaskModal(true)}
                            onDeleteTask={() => setShowDeleteTaskModal(true)}
                            onStatusChange={async () => {
                                await refreshTaskData(task.id);
                            }}
                        />
                    </div>
                </div>
            </div>

            {showDiffViewer && latestSuccessAttempt && (
                <DiffViewerModal
                    attemptId={latestSuccessAttempt.id}
                    taskStatus={normalizedStatus}
                    onClose={() => setShowDiffViewer(false)}
                    onApproved={() => { setShowDiffViewer(false); handleBack(); }}
                />
            )}

            <ConfigureAgentModal
                isOpen={showAgentConfig}
                onClose={() => setShowAgentConfig(false)}
                taskId={task.id}
                taskTitle={task.title}
                onStart={handleStartAgent}
            />

            {/* View Logs Drawer */}
            <ViewLogsModal
                isOpen={showLogsDrawer}
                onClose={() => setShowLogsDrawer(false)}
                task={mapTaskToKanbanTask(task)}
                projectId={projectId}
                initialAttemptId={
                    attempts.length > 0
                        ? attempts.sort(
                              (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
                          )[0].id
                        : null
                }
            />

            <EditTaskModal
                isOpen={showEditTaskModal}
                onClose={() => setShowEditTaskModal(false)}
                task={mapTaskToKanbanTask(task)}
                projectId={task.project_id}
                onSuccess={() => {
                    void refreshTaskData(task.id);
                }}
            />

            <ConfirmModal
                isOpen={showDeleteTaskModal}
                onClose={() => {
                    if (!isDeletingTask) {
                        setShowDeleteTaskModal(false);
                    }
                }}
                onConfirm={handleDeleteTask}
                title="Delete Task"
                message={`Delete "${task.title}"? This action cannot be undone.`}
                confirmText="Delete Task"
                confirmVariant="danger"
                isLoading={isDeletingTask}
            />
        </AppShell>
    );
}

// Helper to map Task to KanbanTask for ViewLogsModal
function mapTaskToKanbanTask(task: Task): KanbanTask {
    const normalizeStatus = (status: string) =>
        status.replace(/([a-z])([A-Z])/g, '$1_$2').toLowerCase();

    return {
        id: task.id,
        title: task.title,
        description: task.description || '',
        status: normalizeStatus(task.status) as KanbanTask['status'],
        priority: (task.metadata?.priority as KanbanTask['priority']) || 'medium',
        type: (task.task_type as KanbanTask['type']) || 'feature',
        agentWorking: undefined,
        assignee: undefined,
        hasIssue: task.metadata?.has_issue === true,
        createdAt: task.created_at,
    };
}
