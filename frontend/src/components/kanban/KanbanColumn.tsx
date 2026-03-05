import { useDroppable } from '@dnd-kit/core';
import { useDraggable } from '@dnd-kit/core';
import { Button } from '../ui/button';
import { Plus, X } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { KanbanColumn as KanbanColumnType } from '../../types/project';
import { TaskCard } from './TaskCard';
import { statusLabels } from '../../utils/statusLabels';

interface KanbanColumnProps {
    column: KanbanColumnType;
    onTaskClick: (id: string, title: string) => void;
    onAddTask?: () => void;
    /** ID of currently selected task (for highlighting) */
    selectedTaskId?: string | null;
    onTaskStart?: (taskId: string) => Promise<void> | void;
    onTaskCancelExecution?: (taskId: string) => Promise<void> | void;
    onTaskDelete?: (taskId: string) => Promise<void> | void;
    onTaskViewDetails?: (taskId: string) => void;
    onTaskEdit?: (taskId: string) => void;
    onTaskNewAttempt?: (taskId: string) => void;
    onTaskRetry?: (taskId: string) => Promise<void> | void;
    /** Whether to show project name chip (when filtering all projects) */
    isAllProjects?: boolean;
    /** Archive a single done task */
    onTaskClose?: (taskId: string) => Promise<void> | void;
    /** Archive all done tasks */
    onCloseAllDone?: () => Promise<void> | void;
}

export function KanbanColumn({
    column,
    onTaskClick,
    onAddTask,
    selectedTaskId,
    onTaskStart,
    onTaskCancelExecution,
    onTaskDelete,
    onTaskViewDetails,
    onTaskEdit,
    onTaskNewAttempt,
    onTaskRetry,
    isAllProjects = false,
    onTaskClose,
    onCloseAllDone,
}: KanbanColumnProps) {
    // Make column a drop target
    const { setNodeRef, isOver } = useDroppable({
        id: `column-${column.status}`,
    });

    // Get status label
    const statusLabel = statusLabels[column.status] || column.title;

    // Map status to CSS variable name for gradient background
    const statusColorVar: Record<string, string> = {
        'todo': '--neutral',
        'in_progress': '--info',
        'in_review': '--warning',
        'blocked': '--destructive',
        'done': '--success',
        'archived': '--neutral',
    };
    const colorVar = statusColorVar[column.status] || '--neutral';

    // Container matching vibe-kanban-reference: flex min-h-40 flex-col
    const containerClasses = cn(
        'flex min-h-40 min-w-0 flex-col',
        isOver && 'outline-primary'
    );

    return (
        <div ref={setNodeRef} className={containerClasses}>
            {/* Column Header - Card with gradient background matching vibe-kanban-reference */}
            <div
                className={cn(
                    'sticky top-0 z-20 flex shrink-0 items-center gap-2 p-3 border-b border-dashed border-border bg-background'
                )}
                style={{
                    backgroundImage: `linear-gradient(hsl(var(${colorVar}) / 0.03), hsl(var(${colorVar}) / 0.03))`,
                }}
            >
                <span className="flex-1 flex items-center gap-2">
                    <div className={cn('h-2 w-2 rounded-full')} style={{ backgroundColor: `hsl(var(${colorVar}))` }} />
                    <p className="m-0 text-sm">{statusLabel}</p>
                </span>
                {onCloseAllDone && column.tasks.length > 0 && (
                    <Button
                        variant="ghost"
                        size="icon"
                        className="m-0 p-0 h-0 text-foreground/50 hover:text-foreground"
                        onClick={onCloseAllDone}
                        aria-label="Close all done tasks"
                        title="Close all done tasks"
                    >
                        <X className="h-4 w-4" />
                    </Button>
                )}
                {onAddTask && (
                    <Button
                        variant="ghost"
                        size="icon"
                        className="m-0 p-0 h-0 text-foreground/50 hover:text-foreground"
                        onClick={onAddTask}
                        aria-label="Add task"
                        title="Add task"
                    >
                        <Plus className="h-4 w-4" />
                    </Button>
                )}
            </div>

            {/* Tasks List - KanbanCards wrapper */}
            <div className="flex flex-1 flex-col">
                {column.tasks.map(task => (
                    <div key={task.id} className="relative group/close">
                        <DraggableTaskCard
                            task={task}
                            onClick={onTaskClick}
                            isSelected={selectedTaskId === task.id}
                            onTaskStart={onTaskStart}
                            onTaskCancelExecution={onTaskCancelExecution}
                            onTaskDelete={onTaskDelete}
                            onTaskViewDetails={onTaskViewDetails}
                            onTaskEdit={onTaskEdit}
                            onTaskNewAttempt={onTaskNewAttempt}
                            onTaskRetry={onTaskRetry}
                            isAllProjects={isAllProjects}
                        />
                        {onTaskClose && (
                            <button
                                onClick={(e) => { e.stopPropagation(); onTaskClose(task.id); }}
                                className="absolute top-2 right-2 opacity-0 group-hover/close:opacity-100 transition-opacity p-0.5 rounded hover:bg-destructive/10 text-foreground/40 hover:text-destructive"
                                title="Archive task"
                                aria-label="Archive task"
                            >
                                <X className="h-3.5 w-3.5" />
                            </button>
                        )}
                    </div>
                ))}
            </div>
        </div>
    );
}

/**
 * DraggableTaskCard - Wrapper to make TaskCard draggable
 */
function DraggableTaskCard({
    task,
    onClick,
    isSelected,
    onTaskStart,
    onTaskCancelExecution,
    onTaskDelete,
    onTaskViewDetails,
    onTaskEdit,
    onTaskNewAttempt,
    onTaskRetry,
    isAllProjects,
}: {
    task: any;
    onClick: (id: string, title: string) => void;
    isSelected: boolean;
    onTaskStart?: (taskId: string) => Promise<void> | void;
    onTaskCancelExecution?: (taskId: string) => Promise<void> | void;
    onTaskDelete?: (taskId: string) => Promise<void> | void;
    onTaskViewDetails?: (taskId: string) => void;
    onTaskEdit?: (taskId: string) => void;
    onTaskNewAttempt?: (taskId: string) => void;
    onTaskRetry?: (taskId: string) => Promise<void> | void;
    isAllProjects?: boolean;
}) {
    const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
        id: task.id,
    });

    return (
        <div
            ref={setNodeRef}
            {...listeners}
            {...attributes}
            className={isDragging ? 'opacity-50' : ''}
        >
            <TaskCard
                task={task}
                onClick={onClick}
                isSelected={isSelected}
                onStart={onTaskStart}
                onCancelExecution={onTaskCancelExecution}
                onDelete={onTaskDelete}
                onViewDetails={onTaskViewDetails}
                onEdit={onTaskEdit}
                onNewAttempt={onTaskNewAttempt}
                onRetry={onTaskRetry}
                isAllProjects={isAllProjects}
            />
        </div>
    );
}
