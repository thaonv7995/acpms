/**
 * PanelHeader - Header component for AgentSessionPanel
 * Shows task title, branch name, connection status, and action buttons
 */

import { memo } from 'react';
import type { KanbanTask } from '../../types/project';

interface PanelHeaderProps {
  task: KanbanTask;
  branchName: string;
  isConnected: boolean;
  status: string;
  onClose: () => void;
  onRefresh: () => void;
}

export const PanelHeader = memo(function PanelHeader({
  task,
  branchName,
  isConnected,
  status,
  onClose,
  onRefresh,
}: PanelHeaderProps) {
  const statusColor =
    {
      idle: 'bg-slate-500',
      running: 'bg-blue-500 animate-pulse',
      completed: 'bg-green-500',
      failed: 'bg-red-500',
      cancelled: 'bg-slate-500',
      waiting_input: 'bg-amber-500 animate-pulse',
    }[status] || 'bg-slate-500';

  return (
    <div className="flex items-center justify-between px-4 py-3 border-b border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/50">
      <div className="flex items-center gap-3 min-w-0 flex-1">
        {/* Status indicator */}
        <span className={`size-2 rounded-full ${statusColor}`} title={status} />

        {/* Task title */}
        <h2 className="text-sm font-semibold text-slate-900 dark:text-white truncate">
          {task.title}
        </h2>

        {/* Branch name pill */}
        <span className="hidden sm:inline-flex items-center gap-1 px-2 py-0.5 rounded bg-slate-200 dark:bg-slate-700 text-xs text-slate-600 dark:text-slate-300 font-mono">
          <span className="material-symbols-outlined text-[14px]">fork_right</span>
          {branchName}
        </span>

        {/* Connection status */}
        {isConnected && (
          <span className="hidden md:flex items-center gap-1 text-xs text-green-500">
            <span className="size-1.5 bg-green-500 rounded-full" />
            Live
          </span>
        )}
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-1">
        <button
          onClick={onRefresh}
          className="p-1.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-500 dark:text-slate-400 transition-colors"
          title="Refresh"
        >
          <span className="material-symbols-outlined text-[18px]">refresh</span>
        </button>
        <button
          className="p-1.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-500 dark:text-slate-400 transition-colors"
          title="View in new tab"
        >
          <span className="material-symbols-outlined text-[18px]">open_in_new</span>
        </button>
        <button
          className="p-1.5 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-500 dark:text-slate-400 transition-colors"
          title="More actions"
        >
          <span className="material-symbols-outlined text-[18px]">more_vert</span>
        </button>
        <button
          onClick={onClose}
          className="p-1.5 rounded hover:bg-red-100 dark:hover:bg-red-900/30 text-slate-500 dark:text-slate-400 hover:text-red-600 dark:hover:text-red-400 transition-colors ml-1"
          title="Close panel (Esc)"
        >
          <span className="material-symbols-outlined text-[18px]">close</span>
        </button>
      </div>
    </div>
  );
});
