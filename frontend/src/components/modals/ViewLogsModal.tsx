// ViewLogsDrawer - Right drawer to view task details, attempts list, and logs
// Consistent with TaskBoardPage split panel pattern
import { useState, useMemo, useCallback, useEffect } from 'react';
import { useGetAttempt } from '../../api/generated/task-attempts/task-attempts';
import { TaskPanel } from '../panels/TaskPanel';
import { TaskAttemptPanel } from '../panels/TaskAttemptPanel';
import { DiffViewer } from '../diff-viewer';
import { GitErrorBanner } from '../panels/GitErrorBanner';
import { TodoPanel } from '../tasks/TodoPanel';
import { NewCard, NewCardHeader } from '../ui/new-card';
import { AttemptHeaderActions } from '../panels/AttemptHeaderActions';
import {
  Breadcrumb,
  BreadcrumbList,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '../ui/breadcrumb';
import { GitOperationsProvider } from '../../contexts/GitOperationsContext';
import { RetryUiProvider } from '../../contexts/RetryUiContext';
import type { LayoutMode } from '../layout/TasksLayout';
import type { KanbanTask } from '../../types/project';
import type { TaskAttempt } from '../../types/task-attempt';

interface ViewLogsModalProps {
    isOpen: boolean;
    onClose: () => void;
    task: KanbanTask;
    projectId?: string;
    initialAttemptId?: string | null; // Optional: auto-select this attempt when opening
}

export function ViewLogsModal({ isOpen, onClose, task, projectId, initialAttemptId }: ViewLogsModalProps) {
    // State for navigation within drawer
    const [selectedAttemptId, setSelectedAttemptId] = useState<string | null>(initialAttemptId || null);
    const [mode, setMode] = useState<LayoutMode>(null);

    // Update selectedAttemptId when initialAttemptId changes (e.g., when modal opens with a specific attempt)
    useEffect(() => {
        if (isOpen && initialAttemptId) {
            setSelectedAttemptId(initialAttemptId);
        } else if (!isOpen) {
            // Reset when modal closes
            setSelectedAttemptId(null);
            setMode(null);
        }
    }, [isOpen, initialAttemptId]);

    // Fetch selected attempt details; only poll when attempt is still active
    const { data: attemptResponse } = useGetAttempt(selectedAttemptId!, {
        query: {
            enabled: !!selectedAttemptId,
            refetchInterval: (query) => {
                const response = query.state.data as { data?: { status?: string } } | undefined;
                const status = response?.data?.status?.toLowerCase();
                const isActive = status === 'running' || status === 'queued';
                return isActive ? 5000 : false;
            },
        },
    });

    // Map API response to TaskAttempt type
    const selectedAttempt: TaskAttempt | null = useMemo(() => {
        if (!selectedAttemptId || !attemptResponse?.data) return null;
        const attempt = attemptResponse.data;
        return {
            id: attempt.id,
            task_id: attempt.task_id,
            branch: (attempt.metadata as { branch?: string })?.branch,
            status: attempt.status.toLowerCase() as TaskAttempt['status'],
            started_at: attempt.started_at ?? undefined,
            completed_at: attempt.completed_at ?? undefined,
            error_message: attempt.error_message ?? undefined,
            created_at: attempt.created_at,
            updated_at: attempt.created_at,
        };
    }, [selectedAttemptId, attemptResponse]);

    // Navigation handlers
    const handleAttemptSelect = useCallback((attemptId: string) => {
        setSelectedAttemptId(attemptId);
        setMode(null); // Reset mode when selecting new attempt
    }, []);

    const handleBackToTask = useCallback(() => {
        setSelectedAttemptId(null);
        setMode(null);
    }, []);

    const handleModeChange = useCallback((newMode: LayoutMode) => {
        setMode(newMode);
    }, []);

    const handleClose = useCallback(() => {
        // Reset state when closing
        setSelectedAttemptId(null);
        setMode(null);
        onClose();
    }, [onClose]);

    const handleCreateAttempt = useCallback(() => {
        // For now, just close the drawer - could open ConfigureAgentModal instead
        handleClose();
    }, [handleClose]);

    // Determine drawer width based on mode
    const drawerWidth = mode === 'diffs' ? 'w-full max-w-6xl' : 'w-full max-w-2xl';
    const isTaskView = !selectedAttemptId;
    const taskTitle = task.title || 'Untitled Task';
    const attemptBranch = selectedAttempt?.branch || (selectedAttempt ? `Attempt #${selectedAttempt.id.slice(0, 8)}` : '');

    // Truncate title for breadcrumbs
    const truncateTitle = (title: string, maxLength = 20) => {
        if (title.length <= maxLength) return title;
        const truncated = title.substring(0, maxLength);
        const lastSpace = truncated.lastIndexOf(' ');
        return lastSpace > 0
            ? `${truncated.substring(0, lastSpace)}...`
            : `${truncated}...`;
    };

    return (
        <>
            {/* Backdrop */}
            <div
                className={`fixed inset-0 z-40 bg-black/40 backdrop-blur-sm transition-opacity duration-300 ${
                    isOpen ? 'opacity-100' : 'opacity-0 pointer-events-none'
                }`}
                onClick={handleClose}
            />

            {/* Drawer */}
            <div
                className={`fixed top-0 right-0 z-50 h-full bg-card border-l border-border shadow-2xl transform transition-all duration-300 ease-out flex flex-col ${
                    isOpen ? 'translate-x-0' : 'translate-x-full'
                } ${drawerWidth}`}
            >
                {/* Header - using NewCardHeader for consistency with ProjectTasksPage */}
                <NewCardHeader
                    className="shrink-0 sticky top-0 z-20 bg-background border-b border-border"
                    actions={
                        !isTaskView && selectedAttempt ? (
                            <AttemptHeaderActions
                                mode={mode}
                                onModeChange={handleModeChange}
                                task={task}
                                attempt={selectedAttempt}
                                onClose={handleClose}
                            />
                        ) : (
                            <AttemptHeaderActions
                                task={task}
                                attempt={null}
                                onClose={handleClose}
                            />
                        )
                    }
                >
                    <div className="mx-auto w-full">
                        <Breadcrumb>
                            <BreadcrumbList>
                                <BreadcrumbItem>
                                    {isTaskView ? (
                                        <BreadcrumbPage>
                                            {truncateTitle(taskTitle)}
                                        </BreadcrumbPage>
                                    ) : (
                                        <BreadcrumbLink
                                            className="cursor-pointer hover:underline"
                                            onClick={(e) => {
                                                e.preventDefault();
                                                e.stopPropagation();
                                                handleBackToTask();
                                            }}
                                            href="#"
                                        >
                                            {truncateTitle(taskTitle)}
                                        </BreadcrumbLink>
                                    )}
                                </BreadcrumbItem>
                                {!isTaskView && (
                                    <>
                                        <BreadcrumbSeparator />
                                        <BreadcrumbItem>
                                            <BreadcrumbPage>
                                                {attemptBranch}
                                            </BreadcrumbPage>
                                        </BreadcrumbItem>
                                    </>
                                )}
                            </BreadcrumbList>
                        </Breadcrumb>
                    </div>
                </NewCardHeader>

                {/* Content */}
                <div className="flex-1 overflow-hidden">
                    {!selectedAttemptId ? (
                        // Task view with attempts list - using TaskPanel for consistency
                        <TaskPanel
                            task={task}
                            projectId={projectId || ''}
                            onAttemptSelect={handleAttemptSelect}
                            onCreateAttempt={handleCreateAttempt}
                        />
                    ) : mode === 'diffs' && selectedAttempt ? (
                        // Attempt + Diffs side by side
                        <GitOperationsProvider attemptId={selectedAttempt.id}>
                            <RetryUiProvider>
                                <div className="h-full flex divide-x divide-border">
                                    <div className="w-1/2 overflow-hidden">
                                        <NewCard className="h-full min-h-0 flex flex-col bg-diagonal-lines bg-muted border-0">
                                            <TaskAttemptPanel task={task} attempt={selectedAttempt}>
                                                {({ logs, followUp, isRunning }) => (
                                                    <>
                                                        <GitErrorBanner />
                                                        <div className="flex-1 min-h-0 flex flex-col">
                                                            <div className="flex-1 min-h-0 flex flex-col">{logs}</div>

                                                            <div className="shrink-0 border-t border-border">
                                                                <div className="mx-auto w-full max-w-[50rem]">
                                                                    <TodoPanel />
                                                                </div>
                                                            </div>

                                                            {!isRunning && (
                                                                <div className="min-h-0 max-h-[50%] border-t border-border overflow-hidden bg-background">
                                                                    <div className="mx-auto w-full max-w-[50rem] h-full min-h-0">
                                                                        {followUp}
                                                                    </div>
                                                                </div>
                                                            )}
                                                        </div>
                                                    </>
                                                )}
                                            </TaskAttemptPanel>
                                        </NewCard>
                                    </div>
                                    <div className="w-1/2 overflow-hidden">
                                        <DiffViewer
                                            attemptId={selectedAttempt.id}
                                            taskTitle={task.title}
                                        />
                                    </div>
                                </div>
                            </RetryUiProvider>
                        </GitOperationsProvider>
                    ) : (
                        // Attempt detail only
                        <GitOperationsProvider attemptId={selectedAttempt?.id}>
                            <RetryUiProvider>
                                <NewCard className="h-full min-h-0 flex flex-col bg-diagonal-lines bg-muted border-0">
                                    <TaskAttemptPanel task={task} attempt={selectedAttempt}>
                                        {({ logs, followUp, isRunning }) => (
                                            <>
                                                <GitErrorBanner />
                                                <div className="flex-1 min-h-0 flex flex-col">
                                                    <div className="flex-1 min-h-0 flex flex-col">{logs}</div>

                                                    <div className="shrink-0 border-t border-border">
                                                        <div className="mx-auto w-full max-w-[50rem]">
                                                            <TodoPanel />
                                                        </div>
                                                    </div>

                                                    {!isRunning && (
                                                        <div className="min-h-0 max-h-[50%] border-t border-border overflow-hidden bg-background">
                                                            <div className="mx-auto w-full max-w-[50rem] h-full min-h-0">
                                                                {followUp}
                                                            </div>
                                                        </div>
                                                    )}
                                                </div>
                                            </>
                                        )}
                                    </TaskAttemptPanel>
                                </NewCard>
                            </RetryUiProvider>
                        </GitOperationsProvider>
                    )}
                </div>
            </div>
        </>
    );
}
