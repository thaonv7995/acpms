/**
 * Status labels and color configuration for kanban board
 * ACPMS has 6 task statuses (vibe-kanban has 5)
 */

export type TaskStatus = 'backlog' | 'todo' | 'in_progress' | 'in_review' | 'blocked' | 'done' | 'archived';

/**
 * Human-readable labels for task statuses
 */
export const statusLabels: Record<TaskStatus, string> = {
  backlog: 'Backlog',
  todo: 'To Do',
  in_progress: 'In Progress',
  in_review: 'In Review',
  blocked: 'Blocked',
  done: 'Done',
  archived: 'Archived',
};

/**
 * Tailwind CSS color classes for board display
 * Includes background, text, and status dot colors
 */
export const statusBoardColors: Record<TaskStatus, {
  bg: string;
  text: string;
  dot: string;
  header: string;
}> = {
  backlog: {
    bg: 'bg-slate-50 dark:bg-slate-900/50',
    text: 'text-slate-700 dark:text-slate-300',
    dot: 'bg-slate-400 dark:bg-slate-500',
    header: 'bg-slate-100 dark:bg-slate-800 border-slate-200 dark:border-slate-700',
  },
  todo: {
    bg: 'bg-slate-50 dark:bg-slate-900/50',
    text: 'text-slate-700 dark:text-slate-300',
    dot: 'bg-slate-400 dark:bg-slate-500',
    header: 'bg-slate-100 dark:bg-slate-800 border-slate-200 dark:border-slate-700',
  },
  in_progress: {
    bg: 'bg-blue-50 dark:bg-blue-900/30',
    text: 'text-blue-700 dark:text-blue-300',
    dot: 'bg-blue-500 dark:bg-blue-400',
    header: 'bg-blue-100 dark:bg-blue-900/50 border-blue-200 dark:border-blue-800',
  },
  in_review: {
    bg: 'bg-amber-50 dark:bg-amber-900/30',
    text: 'text-amber-700 dark:text-amber-300',
    dot: 'bg-amber-500 dark:bg-amber-400',
    header: 'bg-amber-100 dark:bg-amber-900/50 border-amber-200 dark:border-amber-800',
  },
  blocked: {
    bg: 'bg-red-50 dark:bg-red-900/30',
    text: 'text-red-700 dark:text-red-300',
    dot: 'bg-red-500 dark:bg-red-400',
    header: 'bg-red-100 dark:bg-red-900/50 border-red-200 dark:border-red-800',
  },
  done: {
    bg: 'bg-green-50 dark:bg-green-900/30',
    text: 'text-green-700 dark:text-green-300',
    dot: 'bg-green-500 dark:bg-green-400',
    header: 'bg-green-100 dark:bg-green-900/50 border-green-200 dark:border-green-800',
  },
  archived: {
    bg: 'bg-gray-50 dark:bg-gray-900/30',
    text: 'text-gray-600 dark:text-gray-400',
    dot: 'bg-gray-400 dark:bg-gray-500',
    header: 'bg-gray-100 dark:bg-gray-800 border-gray-200 dark:border-gray-700',
  },
};

/**
 * Canonical order of statuses on the kanban board (left to right)
 */
export const statusOrder: TaskStatus[] = [
  'backlog',
  'todo',
  'in_progress',
  'in_review',
  'blocked',
  'done',
  'archived',
];

/**
 * Get color configuration for a status
 * @param status - The task status
 * @returns Color configuration object or undefined if status not found
 */
export function getStatusColor(status: TaskStatus) {
  return statusBoardColors[status];
}

/**
 * Get display label for a status
 * @param status - The task status
 * @returns Human-readable label
 */
export function getStatusLabel(status: TaskStatus): string {
  return statusLabels[status] || status;
}

/**
 * Validate if a string is a valid task status
 * @param value - Value to validate
 * @returns True if valid status, false otherwise
 */
export function isValidStatus(value: unknown): value is TaskStatus {
  return typeof value === 'string' && value in statusLabels;
}
