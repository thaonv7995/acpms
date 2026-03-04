import { MoreVertical, Edit, Eye, Trash2, Play, RotateCcw } from 'lucide-react';
import type { KanbanTask } from '@/types/project';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';

interface TaskCardDropdownProps {
  task: KanbanTask;
  onEdit?: (taskId: string) => void;
  onViewDetails?: (taskId: string) => void;
  onDelete?: (taskId: string) => void;
  onNewAttempt?: (taskId: string) => void;
  onRetry?: (taskId: string) => void;
}

export function TaskCardDropdown({
  task,
  onEdit,
  onViewDetails,
  onDelete,
  onNewAttempt,
  onRetry,
}: TaskCardDropdownProps) {
  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
  };

  // Check if task has attempts (using latestAttemptId as proxy)
  const hasAttempts = !!task.latestAttemptId;
  // Assume last attempt failed if status is not in_progress or done
  const lastAttemptFailed = task.status !== 'in_progress' && task.status !== 'done' && hasAttempts;

  // Edit Task: only when todo or in_review (not merged). Disabled when in_progress or done.
  const canEditTask =
    task.status === 'todo' || task.status === 'in_review';

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild onClick={handleClick}>
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0"
          aria-label="Open task menu"
        >
          <MoreVertical className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>

      <DropdownMenuContent align="end" className="w-48">
        <DropdownMenuItem onClick={(e) => {
          e.stopPropagation();
          onViewDetails?.(task.id);
        }}>
          <Eye className="h-4 w-4 mr-2" />
          View Details
        </DropdownMenuItem>

        {canEditTask && (
          <DropdownMenuItem onClick={(e) => {
            e.stopPropagation();
            onEdit?.(task.id);
          }}>
            <Edit className="h-4 w-4 mr-2" />
            Edit Task
          </DropdownMenuItem>
        )}

        {hasAttempts && (
          <DropdownMenuItem onClick={(e) => {
            e.stopPropagation();
            onNewAttempt?.(task.id);
          }}>
            <Play className="h-4 w-4 mr-2" />
            New Attempt
          </DropdownMenuItem>
        )}

        {lastAttemptFailed && (
          <DropdownMenuItem onClick={(e) => {
            e.stopPropagation();
            onRetry?.(task.id);
          }}>
            <RotateCcw className="h-4 w-4 mr-2" />
            Retry
          </DropdownMenuItem>
        )}

        <DropdownMenuSeparator />

        <DropdownMenuItem
          onClick={(e) => {
            e.stopPropagation();
            onDelete?.(task.id);
          }}
          className="text-destructive focus:text-destructive"
        >
          <Trash2 className="h-4 w-4 mr-2" />
          Delete Task
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
