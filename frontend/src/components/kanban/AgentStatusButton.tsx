/**
 * AgentStatusButton - Button component showing agent execution status on TaskCard
 *
 * States:
 * - idle      -> [Start Agent] - Ready to start
 * - queued    -> [Queued...] - Waiting in queue
 * - running   -> [Running * 2 files] - Actively executing (animated)
 * - success   -> [Complete * +45 -3] - Finished successfully
 * - failed    -> [Failed * View logs] - Execution failed
 * - in_review -> [Review * 3 files] - Waiting for review
 */

import { clsx } from 'clsx';

export type AgentStatus = 'idle' | 'queued' | 'running' | 'success' | 'failed' | 'in_review';

export interface AgentStatusButtonProps {
  /** Current agent status */
  status: AgentStatus;
  /** Number of files changed (for running/success/in_review) */
  filesChanged?: number;
  /** Lines added (for success state) */
  additions?: number;
  /** Lines removed (for success state) */
  deletions?: number;
  /** Click handler - opens agent session panel */
  onClick?: (e: React.MouseEvent) => void;
  /** Whether button is disabled */
  disabled?: boolean;
  /** Compact mode (smaller for mobile) */
  compact?: boolean;
}

export function AgentStatusButton({
  status,
  filesChanged = 0,
  additions = 0,
  deletions = 0,
  onClick,
  disabled = false,
  compact = false,
}: AgentStatusButtonProps) {
  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation(); // Prevent card click
    onClick?.(e);
  };

  // Status-specific styling
  const getStatusStyles = () => {
    switch (status) {
      case 'idle':
        return {
          bg: 'bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700',
          border: 'border-slate-200 dark:border-slate-700',
          text: 'text-slate-600 dark:text-slate-300',
          icon: 'text-slate-500 dark:text-slate-400',
        };
      case 'queued':
        return {
          bg: 'bg-amber-50 dark:bg-amber-900/20',
          border: 'border-amber-200 dark:border-amber-800/50',
          text: 'text-amber-700 dark:text-amber-300',
          icon: 'text-amber-500',
        };
      case 'running':
        return {
          bg: 'bg-blue-50 dark:bg-blue-900/20',
          border: 'border-blue-300 dark:border-blue-700/50',
          text: 'text-blue-700 dark:text-blue-300',
          icon: 'text-blue-500',
        };
      case 'success':
        return {
          bg: 'bg-green-50 dark:bg-green-900/20',
          border: 'border-green-200 dark:border-green-800/50',
          text: 'text-green-700 dark:text-green-300',
          icon: 'text-green-500',
        };
      case 'failed':
        return {
          bg: 'bg-red-50 dark:bg-red-900/20',
          border: 'border-red-200 dark:border-red-800/50',
          text: 'text-red-700 dark:text-red-300',
          icon: 'text-red-500',
        };
      case 'in_review':
        return {
          bg: 'bg-purple-50 dark:bg-purple-900/20',
          border: 'border-purple-200 dark:border-purple-800/50',
          text: 'text-purple-700 dark:text-purple-300',
          icon: 'text-purple-500',
        };
    }
  };

  const styles = getStatusStyles();

  // Render status-specific content
  const renderContent = () => {
    switch (status) {
      case 'idle':
        return (
          <>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[14px]' : 'text-[16px]', styles.icon)}>
              smart_toy
            </span>
            <span className="flex-1 text-left">Start Agent</span>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[12px]' : 'text-[14px]', styles.icon)}>
              play_arrow
            </span>
          </>
        );

      case 'queued':
        return (
          <>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[14px]' : 'text-[16px]', styles.icon)}>
              smart_toy
            </span>
            <span className="flex-1 text-left">Queued...</span>
            <span className={clsx('material-symbols-outlined animate-pulse', compact ? 'text-[12px]' : 'text-[14px]', styles.icon)}>
              hourglass_empty
            </span>
          </>
        );

      case 'running':
        return (
          <>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[14px]' : 'text-[16px]', styles.icon)}>
              smart_toy
            </span>
            <span className="flex-1 text-left flex items-center gap-1">
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-blue-500"></span>
              </span>
              Running
              {filesChanged > 0 && (
                <span className="text-blue-500 dark:text-blue-400 ml-1">
                  {filesChanged} {filesChanged === 1 ? 'file' : 'files'}
                </span>
              )}
            </span>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[12px]' : 'text-[14px]', styles.icon)}>
              play_arrow
            </span>
          </>
        );

      case 'success':
        return (
          <>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[14px]' : 'text-[16px]', styles.icon)}>
              smart_toy
            </span>
            <span className="flex-1 text-left flex items-center gap-1">
              Complete
              {(additions > 0 || deletions > 0) && (
                <span className="ml-1">
                  <span className="text-green-600 dark:text-green-400">+{additions}</span>
                  {' '}
                  <span className="text-red-600 dark:text-red-400">-{deletions}</span>
                </span>
              )}
            </span>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[12px]' : 'text-[14px]', styles.icon)}>
              check_circle
            </span>
          </>
        );

      case 'failed':
        return (
          <>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[14px]' : 'text-[16px]', styles.icon)}>
              smart_toy
            </span>
            <span className="flex-1 text-left">Failed</span>
            <span className="text-xs opacity-75">View logs</span>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[12px]' : 'text-[14px]', styles.icon)}>
              error
            </span>
          </>
        );

      case 'in_review':
        return (
          <>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[14px]' : 'text-[16px]', styles.icon)}>
              smart_toy
            </span>
            <span className="flex-1 text-left flex items-center gap-1">
              Review
              {filesChanged > 0 && (
                <span className="text-purple-500 dark:text-purple-400 ml-1">
                  {filesChanged} {filesChanged === 1 ? 'file' : 'files'}
                </span>
              )}
            </span>
            <span className={clsx('material-symbols-outlined', compact ? 'text-[12px]' : 'text-[14px]', styles.icon)}>
              visibility
            </span>
          </>
        );
    }
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={disabled}
      className={clsx(
        'w-full flex items-center gap-2 rounded-lg border transition-all duration-150',
        compact ? 'px-2 py-1.5 text-[10px]' : 'px-3 py-2 text-xs',
        'font-medium',
        styles.bg,
        styles.border,
        styles.text,
        disabled && 'opacity-50 cursor-not-allowed',
        !disabled && 'cursor-pointer'
      )}
    >
      {renderContent()}
    </button>
  );
}

/**
 * Helper function to determine agent status from task data
 */
export function getAgentStatusFromTask(task: {
  status: string;
  agentWorking?: { name: string; progress: number } | null;
}): AgentStatus {
  // If agent is currently working
  if (task.agentWorking) {
    return 'running';
  }

  // Based on task status
  switch (task.status) {
    case 'in_review':
      return 'in_review';
    case 'done':
      return 'success';
    case 'in_progress':
      // Could be running or queued - default to running
      return 'running';
    default:
      return 'idle';
  }
}
