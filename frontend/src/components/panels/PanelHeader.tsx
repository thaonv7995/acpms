import { X } from 'lucide-react';
import { IconButton } from '@mui/material';
import { PanelModeToggle } from './PanelModeToggle';
import type { LayoutMode } from '../layout/TasksLayout';

interface Task {
  id: string;
  title: string;
}

interface TaskAttempt {
  id: string;
  branch?: string;
  status: string;
}

interface PanelHeaderProps {
  task: Task;
  attempt: TaskAttempt | null;
  mode: LayoutMode;
  onModeChange: (mode: LayoutMode) => void;
  onClose: () => void;
  onBackToTask?: () => void;
}

export function PanelHeader({
  task,
  attempt,
  mode,
  onModeChange,
  onClose,
  onBackToTask,
}: PanelHeaderProps) {
  const isTaskView = !attempt;

  return (
    <div className="flex items-center justify-between px-4 py-3 gap-4">
      {/* Breadcrumb */}
      <nav className="flex items-center gap-2 text-sm min-w-0 flex-1">
        {isTaskView ? (
          // Task view: title is not clickable
          <span className="text-slate-700 dark:text-slate-200 font-medium truncate max-w-[200px]">
            {task.title}
          </span>
        ) : (
          // Attempt view: title is clickable to go back
          <button
            onClick={onBackToTask}
            className="text-slate-500 dark:text-slate-400 hover:text-primary dark:hover:text-primary truncate max-w-[200px] transition-colors"
          >
            {task.title}
          </button>
        )}
        {attempt && (
          <>
            <span className="text-slate-400 dark:text-slate-500 flex-shrink-0">/</span>
            <span className="text-slate-700 dark:text-slate-200 font-medium truncate">
              {attempt.branch || `Attempt #${attempt.id.slice(0, 8)}`}
            </span>
          </>
        )}
      </nav>

      {/* Mode Toggle (only when attempt selected) */}
      {attempt && (
        <div className="flex items-center gap-2 flex-shrink-0">
          <PanelModeToggle mode={mode} onModeChange={onModeChange} />
        </div>
      )}

      {/* Close Button */}
      <IconButton
        onClick={onClose}
        size="small"
        className="flex-shrink-0"
        sx={{
          color: 'text.secondary',
          '&:hover': {
            backgroundColor: 'action.hover',
          },
        }}
      >
        <X className="h-5 w-5" />
      </IconButton>
    </div>
  );
}
