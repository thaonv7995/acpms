import { useState } from 'react';
import { updateTaskStatus, TaskStatus } from '../../api/tasks';
import { logger } from '@/lib/logger';

interface TaskMetadataSidebarProps {
    taskId: string;
    status: TaskStatus;
    priority: string;
    type: string;
    createdAt: string;
    onStatusChange?: () => void;
    onEditTask?: () => void;
    onDeleteTask?: () => void;
}

const priorityColors: Record<string, string> = {
    low: 'bg-muted text-muted-foreground',
    Low: 'bg-muted text-muted-foreground',
    medium: 'bg-blue-100 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400',
    Medium: 'bg-blue-100 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400',
    high: 'bg-orange-100 text-orange-600 dark:bg-orange-500/20 dark:text-orange-400',
    High: 'bg-orange-100 text-orange-600 dark:bg-orange-500/20 dark:text-orange-400',
    critical: 'bg-red-100 text-red-600 dark:bg-red-500/20 dark:text-red-400',
    Critical: 'bg-red-100 text-red-600 dark:bg-red-500/20 dark:text-red-400',
};

const allowedTransitions: Record<TaskStatus, TaskStatus[]> = {
    backlog: ['todo', 'in_progress'],
    todo: ['in_progress', 'done', 'archived'],
    in_progress: ['backlog', 'todo', 'in_review', 'done'],
    in_review: ['in_progress', 'done'],
    blocked: ['backlog', 'todo', 'in_progress'],
    done: ['in_progress', 'archived'],
    archived: ['backlog', 'in_progress'],
};

const transitionLabels: Record<TaskStatus, string> = {
    backlog: 'Move to Backlog',
    todo: 'Move to To Do',
    in_progress: 'Move to In Progress',
    in_review: 'Move to In Review',
    blocked: 'Move to Blocked',
    done: 'Move to Done',
    archived: 'Archive Task',
};

export function TaskMetadataSidebar({
    taskId,
    status,
    priority,
    type,
    createdAt,
    onStatusChange,
    onEditTask,
    onDeleteTask,
}: TaskMetadataSidebarProps) {
    const [isUpdating, setIsUpdating] = useState(false);
    const canEditTask =
        status === 'backlog' || status === 'todo' || status === 'in_review';

    const handleStatusChange = async (newStatus: string) => {
        if (!newStatus || isUpdating) return;

        try {
            setIsUpdating(true);
            await updateTaskStatus(taskId, newStatus as TaskStatus);
            onStatusChange?.();
        } catch (err) {
            logger.error('Failed to update status:', err);
        } finally {
            setIsUpdating(false);
        }
    };

    return (
        <div className="flex flex-col gap-6">
            {/* Metadata */}
            <div className="bg-card border border-border rounded-xl p-6">
                <h3 className="text-xs font-bold text-card-foreground uppercase tracking-wider mb-4">
                    Details
                </h3>
                <div className="space-y-4">
                    <div>
                        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider block mb-1.5">Priority</span>
                        <div>
                            <span className={`px-2.5 py-1 rounded text-xs font-medium ${priorityColors[priority] || priorityColors['medium']}`}>
                                {priority.toUpperCase()}
                            </span>
                        </div>
                    </div>
                    <div>
                        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider block mb-1.5">Type</span>
                        <div className="text-sm text-card-foreground capitalize">
                            {(type || 'feature').replace('_', ' ')}
                        </div>
                    </div>
                    <div>
                        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider block mb-1.5">Assignee</span>
                        <div className="flex items-center gap-2">
                            <div className="size-7 rounded-full bg-muted flex items-center justify-center">
                                <span className="material-symbols-outlined text-muted-foreground text-[16px]">person</span>
                            </div>
                            <span className="text-sm text-muted-foreground">Unassigned</span>
                        </div>
                    </div>
                    <div>
                        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider block mb-1.5">Created</span>
                        <div className="text-sm text-card-foreground">
                            {new Date(createdAt).toLocaleDateString()}
                        </div>
                    </div>
                </div>
            </div>

            {/* Quick Actions */}
            <div className="bg-card border border-border rounded-xl p-6">
                <h3 className="text-xs font-bold text-card-foreground uppercase tracking-wider mb-4">
                    Actions
                </h3>
                <div className="space-y-2">
                    <select
                        className="w-full bg-card border border-border text-card-foreground text-sm rounded-lg py-2 px-3 focus:ring-2 focus:ring-primary/20 focus:border-primary disabled:opacity-50"
                        defaultValue=""
                        disabled={isUpdating}
                        onChange={(e) => handleStatusChange(e.target.value)}
                    >
                        <option value="" disabled>Change Status</option>
                        {(allowedTransitions[status] || []).map((nextStatus) => (
                            <option key={nextStatus} value={nextStatus}>
                                {transitionLabels[nextStatus]}
                            </option>
                        ))}
                    </select>
                    {canEditTask && onEditTask && (
                        <button
                            onClick={onEditTask}
                            className="w-full py-2 px-3 text-sm text-muted-foreground hover:bg-muted rounded-lg transition-colors text-left flex items-center gap-2"
                        >
                            <span className="material-symbols-outlined text-[16px]">edit</span>
                            Edit Task
                        </button>
                    )}
                    {onDeleteTask && (
                        <button
                            onClick={onDeleteTask}
                            className="w-full py-2 px-3 text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/20 rounded-lg transition-colors text-left flex items-center gap-2"
                        >
                            <span className="material-symbols-outlined text-[16px]">delete</span>
                            Delete Task
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}
