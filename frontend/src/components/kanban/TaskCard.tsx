import React from 'react';
import {
  Loader2,
  XCircle,
  Link,
  FileText,
  Clock,
  AlertTriangle,
  Play,
  Square,
  Eye,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import type { KanbanTask } from '@/types/project';
import { TaskCardHeader } from './TaskCardHeader';
import { TaskCardDropdown } from './TaskCardDropdown';
import { Card } from '../ui/card';
import { logger } from '@/lib/logger';
import { getKanbanDisplayTitle } from '@/utils/taskTitle';

// Helper to format time ago (e.g., "3d", "2h", "5m")
function formatTimeAgo(dateString: string | undefined): string {
  if (!dateString) return 'now';
  
  const date = new Date(dateString);
  // Check if date is valid
  if (isNaN(date.getTime())) return 'now';
  
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  
  // Handle negative diff (future dates)
  if (diffMs < 0) return 'now';
  
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffDays > 0) return `${diffDays}d`;
  if (diffHours > 0) return `${diffHours}h`;
  if (diffMins > 0) return `${diffMins}m`;
  return 'now';
}

interface TaskCardProps {
  task: KanbanTask;
  isSelected?: boolean;
  isShared?: boolean;
  onClick: (id: string, title: string) => void;
  onAutoScroll?: (ref: HTMLDivElement) => void;
  onEdit?: (taskId: string) => void;
  onViewDetails?: (taskId: string) => void;
  onDelete?: (taskId: string) => void;
  onNewAttempt?: (taskId: string) => void;
  onRetry?: (taskId: string) => void;
  onStart?: (taskId: string) => Promise<void> | void;
  onCancelExecution?: (taskId: string) => Promise<void> | void;
  /** Whether to show project name chip (when filtering all projects) */
  isAllProjects?: boolean;
}

const MAX_DESCRIPTION_LENGTH = 130;
const MAX_PROJECT_NAME_LENGTH = 15; // Show first 15 characters, then "..."

function formatTaskTypeLabel(type: KanbanTask['type']): string {
  return type.replace('_', ' ');
}

export function TaskCard({
  task,
  isSelected,
  isShared,
  onClick,
  onAutoScroll,
  onEdit,
  onViewDetails,
  onDelete,
  onNewAttempt,
  onRetry,
  onStart,
  onCancelExecution,
  isAllProjects = false,
}: TaskCardProps) {
  const cardRef = React.useRef<HTMLDivElement>(null);
  const [pendingAction, setPendingAction] = React.useState<
    'start' | 'cancel' | null
  >(null);

  // Auto-scroll to card when selected
  React.useEffect(() => {
    if (isSelected && cardRef.current && onAutoScroll) {
      onAutoScroll(cardRef.current);
    }
  }, [isSelected, onAutoScroll]);

  const truncatedDescription = task.description
    ? task.description.length > MAX_DESCRIPTION_LENGTH
      ? task.description.slice(0, MAX_DESCRIPTION_LENGTH) + '...'
      : task.description
    : null;

  // Status indicators
  const hasInProgressAttempt = task.status === 'in_progress';
  const lastAttemptFailed = task.status !== 'in_progress' && task.status !== 'done' && !!task.latestAttemptId;
  const hasParentTask = false; // TODO: Add parent_workspace_id to KanbanTask type when backend supports it
  const isInProgressTask = task.status === 'in_progress';
  const canStartTask = !isInProgressTask && task.status !== 'backlog';
  const isActionPending = pendingAction !== null;

  // Executor from agentWorking field
  const executor = task.agentWorking?.name;
  const displayTitle = getKanbanDisplayTitle(task.title);

  const stopCardClick = (event: React.SyntheticEvent) => {
    event.stopPropagation();
  };

  const runAction = async (
    event: React.MouseEvent<HTMLButtonElement>,
    action: 'start' | 'cancel',
    handler?: (taskId: string) => Promise<void> | void
  ) => {
    event.stopPropagation();
    if (!handler || isActionPending) return;

    try {
      setPendingAction(action);
      await handler(task.id);
    } catch (error) {
      logger.error(`Failed to ${action} task ${task.id}:`, error);
    } finally {
      setPendingAction(null);
    }
  };

  return (
    <Card
      ref={cardRef}
      onClick={() => onClick(task.id, displayTitle)}
      className={cn(
        'group p-3 outline-none flex-col space-y-2 cursor-pointer',
        // Border to separate tasks
        'border-b border-border',
        // Remove default Card styles (rounded-lg, shadow-sm, hardcoded colors)
        'rounded-none border-x-0 border-t-0 shadow-none',
        // Override hardcoded colors with CSS variables - use !important to override Card defaults
        '!bg-card !text-card-foreground',
        // Selected state
        isSelected && 'ring-2 ring-secondary-foreground ring-inset',
        // Shared task indicator (3px left border)
        isShared && 'relative overflow-hidden pl-5 before:absolute before:left-0 before:top-0 before:bottom-0 before:w-[3px] before:bg-card-foreground before:content-[""]'
      )}
    >
      <div className="flex flex-col gap-2">
        {/* Header with title and action icons */}
        <TaskCardHeader
          title={displayTitle}
          right={
            <div className="flex items-center gap-1">
              {/* Start action (when not in progress) */}
              {canStartTask && (
                <button
                  type="button"
                  className="h-6 w-6 inline-flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  aria-label="Start task"
                  title="Start task execution"
                  onPointerDown={stopCardClick}
                  onMouseDown={stopCardClick}
                  onTouchStart={stopCardClick}
                  onClick={(event) => runAction(event, 'start', onStart)}
                  disabled={isActionPending || !onStart}
                >
                  {pendingAction === 'start' ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Play className="w-4 h-4" />
                  )}
                </button>
              )}

              {/* Cancel action */}
              {isInProgressTask ? (
                <button
                  type="button"
                  className="h-6 w-6 inline-flex items-center justify-center rounded text-destructive/80 hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  aria-label="Cancel task run"
                  title="Cancel current run"
                  onPointerDown={stopCardClick}
                  onMouseDown={stopCardClick}
                  onTouchStart={stopCardClick}
                  onClick={(event) => runAction(event, 'cancel', onCancelExecution)}
                  disabled={isActionPending || !onCancelExecution}
                >
                  {pendingAction === 'cancel' ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Square className="w-4 h-4" />
                  )}
                </button>
              ) : null}

              {/* View details quick action */}
              <button
                type="button"
                className="h-6 w-6 inline-flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                aria-label="View task details"
                title="View task details"
                onPointerDown={stopCardClick}
                onMouseDown={stopCardClick}
                onTouchStart={stopCardClick}
                onClick={(event) => {
                  event.stopPropagation();
                  onViewDetails?.(task.id);
                }}
                disabled={!onViewDetails}
              >
                <Eye className="w-4 h-4" />
              </button>

              {/* In Progress Indicator */}
              {hasInProgressAttempt && (
                <Loader2 className="w-4 h-4 text-info animate-spin" aria-label="Task in progress" />
              )}

              {/* Failed Indicator */}
              {lastAttemptFailed && !hasInProgressAttempt && (
                <XCircle className="w-4 h-4 text-destructive" aria-label="Last attempt failed" />
              )}

              {/* Has Issue Indicator (preflight blocked, references missing, etc.) */}
              {task.hasIssue && !hasInProgressAttempt && (
                <span title="Task has issues that need resolution before execution">
                  <AlertTriangle
                    className="w-4 h-4 text-warning"
                    aria-label="Task has blocking issues"
                  />
                </span>
              )}

              {/* Has Parent Task Indicator */}
              {hasParentTask && (
                <Link className="w-4 h-4 text-muted-foreground" aria-label="Task has parent task" />
              )}

              {/* Dropdown Menu */}
              <TaskCardDropdown
                task={task}
                onEdit={onEdit}
                onViewDetails={onViewDetails}
                onDelete={onDelete}
                onNewAttempt={onNewAttempt}
                onRetry={onRetry}
              />
            </div>
          }
        />

        {/* Description */}
        {truncatedDescription && (
          <p className="text-sm text-muted-foreground line-clamp-1">
            {truncatedDescription}
          </p>
        )}

        {/* Project name chip and Task type tag - in the same row */}
        {(isAllProjects && task.projectName) || task.type ? (
          <div className="flex items-center gap-2 flex-wrap">
            {/* Project name chip - only show when filtering all projects */}
            {isAllProjects && task.projectName && (
              <span
                className="px-2 py-0.5 rounded text-xs font-medium bg-muted text-card-foreground border border-border truncate max-w-[120px]"
                title={task.projectName}
              >
                {task.projectName.length > MAX_PROJECT_NAME_LENGTH
                  ? `${task.projectName.slice(0, MAX_PROJECT_NAME_LENGTH)}...`
                  : task.projectName}
              </span>
            )}

            {/* Task type tag */}
            {task.type && (
              <span className={cn(
                'px-2 py-0.5 rounded text-xs font-medium',
                task.type === 'feature' && 'bg-purple-100 dark:bg-purple-900/30 text-purple-600 dark:text-purple-300',
                task.type === 'bug' && 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-300',
                task.type === 'hotfix' && 'bg-rose-100 dark:bg-rose-900/30 text-rose-600 dark:text-rose-300',
                task.type === 'refactor' && 'bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-300',
                task.type === 'docs' && 'bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-300',
                task.type === 'test' && 'bg-indigo-100 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-300',
                task.type === 'chore' && 'bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400',
                task.type === 'spike' && 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300',
                task.type === 'small_task' && 'bg-zinc-100 dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300',
                task.type === 'deploy' && 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-600 dark:text-emerald-300',
                task.type === 'init' && 'bg-gray-100 dark:bg-gray-900/30 text-gray-600 dark:text-gray-400'
              )}>
                {formatTaskTypeLabel(task.type)}
              </span>
            )}
          </div>
        ) : null}

        {/* Metadata row - Vibe-Kanban style */}
        <div className="flex items-center gap-4 text-xs text-muted-foreground">
          {/* Documents/Attempts count */}
          {task.latestAttemptId && (
            <span className="flex items-center gap-1" title="Attempts">
              <FileText className="w-3 h-3" />
              {task.attemptCount || 1}
            </span>
          )}

          {/* Time since created */}
          <span 
            className="flex items-center gap-1" 
            title={task.createdAt ? `Created ${new Date(task.createdAt).toLocaleString()}` : 'Created'}
          >
            <Clock className="w-3 h-3" />
            {formatTimeAgo(task.createdAt)}
          </span>

          {/* Priority indicator (if high/critical) */}
          {task.priority && (task.priority === 'high' || task.priority === 'critical') && (
            <span className={cn(
              "flex items-center gap-1 font-medium",
              task.priority === 'critical' && 'text-destructive',
              task.priority === 'high' && 'text-warning'
            )} title={`Priority: ${task.priority}`}>
              <AlertTriangle className="w-3 h-3" />
              {task.priority}
            </span>
          )}

          {/* Executor (if running) */}
          {executor && hasInProgressAttempt && (
            <span className="flex items-center gap-1 text-info font-medium" title="Running by">
              {executor}
            </span>
          )}
        </div>
      </div>
    </Card>
  );
}
