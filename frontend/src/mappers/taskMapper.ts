/**
 * Task Mapper - Transform backend TaskDto to frontend KanbanTask
 *
 * Handles type conversion and mapping between backend and frontend models
 */

import type { TaskDto } from '../api/generated/models';
import type { KanbanTask, KanbanColumn } from '../types/project';

export type CreatedDateFilter =
  | 'all'
  | 'today'
  | 'this_week'
  | 'this_month'
  | 'last_30_days';

// Status mapping: Backend status → Frontend status
// Backend returns PascalCase (Todo, InProgress, Done) or snake_case
const statusMap: Record<string, KanbanTask['status']> = {
  // PascalCase from API
  Todo: 'todo',
  InProgress: 'in_progress',
  InReview: 'in_review',
  Blocked: 'todo',
  Done: 'done',
  Archived: 'archived',
  // Lowercase fallbacks
  todo: 'todo',
  pending: 'todo',
  backlog: 'todo',
  in_progress: 'in_progress',
  in_review: 'in_review',
  testing: 'in_progress',
  done: 'done',
  completed: 'done',
  closed: 'done',
  archived: 'archived',
};

// Task type mapping: Backend task_type → Frontend type
// Backend returns PascalCase (Feature, Bug, etc.)
const typeMap: Record<string, KanbanTask['type']> = {
  // PascalCase from API
  Feature: 'feature',
  Bug: 'bug',
  Hotfix: 'hotfix',
  Refactor: 'refactor',
  Docs: 'docs',
  Test: 'test',
  Chore: 'chore',
  Spike: 'spike',
  SmallTask: 'small_task',
  Deploy: 'deploy',
  Init: 'init',
  // Lowercase fallbacks
  feature: 'feature',
  bug: 'bug',
  hotfix: 'hotfix',
  refactor: 'refactor',
  docs: 'docs',
  test: 'test',
  chore: 'chore',
  spike: 'spike',
  small_task: 'small_task',
  deploy: 'deploy',
  init: 'init',
  task: 'feature', // default
};

// Priority colors for assignee avatars
const priorityColors: Record<string, string> = {
  critical: 'bg-red-500',
  high: 'bg-orange-500',
  medium: 'bg-yellow-500',
  low: 'bg-slate-500',
};

/**
 * Map backend TaskDto to frontend KanbanTask
 */
export function mapBackendTask(
  task: TaskDto,
  usersMap?: Map<string, { name: string }>,
  projectsMap?: Map<string, { name: string }>
): KanbanTask {
  // Extract priority from metadata (default to medium)
  const priority = (task.metadata?.priority as KanbanTask['priority']) || 'medium';

  // Map status (API returns PascalCase like "InProgress", statusMap handles both)
  const status = statusMap[task.status] || statusMap[task.status.toLowerCase()] || 'todo';

  // Map type (API returns PascalCase like "Feature", typeMap handles both)
  const type = typeMap[task.task_type] || typeMap[task.task_type.toLowerCase()] || 'feature';

  // Build assignee if assigned_to exists
  let assignee: KanbanTask['assignee'] | undefined;
  if (task.assigned_to) {
    const user = usersMap?.get(task.assigned_to);
    const name = user?.name || 'Unknown';
    assignee = {
      id: task.assigned_to,
      initial: generateInitials(name),
      color: priorityColors[priority] || 'bg-slate-500',
    };
  }

  // Extract agent working info from metadata if available
  let agentWorking: KanbanTask['agentWorking'] | undefined;
  if (task.metadata?.agent_name) {
    agentWorking = {
      name: task.metadata.agent_name as string,
      progress: (task.metadata.agent_progress as number) || 0,
    };
  }

  // Get project name from projects map if available
  const projectName = projectsMap?.get(task.project_id)?.name;

  return {
    id: task.id,
    title: task.title,
    description: task.description || undefined,
    metadata: task.metadata as Record<string, unknown> | undefined,
    type,
    status,
    priority,
    progress: task.metadata?.progress as number | undefined,
    assignee,
    agentWorking,
    attachments: task.metadata?.attachments_count as number | undefined,
    hasIssue: task.metadata?.has_issue === true,
    latestAttemptId: task.latest_attempt_id || undefined,
    projectId: task.project_id,
    projectName,
    createdAt: task.created_at,
    attemptCount: task.metadata?.attempt_count as number | undefined,
  };
}

/**
 * Map array of backend tasks to frontend KanbanTasks
 */
export function mapBackendTasks(
  tasks: TaskDto[],
  usersMap?: Map<string, { name: string }>,
  projectsMap?: Map<string, { name: string }>
): KanbanTask[] {
  return tasks.map((task) => mapBackendTask(task, usersMap, projectsMap));
}

/**
 * Group tasks into Kanban columns
 */
export function groupTasksIntoColumns(tasks: KanbanTask[]): KanbanColumn[] {
  const columns: KanbanColumn[] = [
    {
      id: 'col-backlog',
      title: 'BACKLOG',
      status: 'todo',
      color: 'slate',
      tasks: [],
    },
    {
      id: 'col-in-progress',
      title: 'AGENT PROCESSING',
      status: 'in_progress',
      color: 'blue',
      tasks: [],
    },
    {
      id: 'col-in-review',
      title: 'IN REVIEW',
      status: 'in_review',
      color: 'yellow',
      tasks: [],
    },
    {
      id: 'col-done',
      title: 'COMPLETED',
      status: 'done',
      color: 'green',
      tasks: [],
    },
  ];

  // Group tasks by status
  for (const task of tasks) {
    const column = columns.find((col) => col.status === task.status);
    if (column) {
      column.tasks.push(task);
    } else {
      // Default to backlog if status doesn't match
      columns[0].tasks.push(task);
    }
  }

  return columns;
}

/**
 * Apply filters to tasks
 */
export function applyTaskFilters(
  tasks: KanbanTask[],
  filters: { agentOnly?: boolean; search?: string; createdDate?: CreatedDateFilter }
): KanbanTask[] {
  let filtered = [...tasks];

  if (filters.agentOnly) {
    // "Execution only" excludes support task types (docs, spike, init).
    const SUPPORT_TASK_TYPES: string[] = ['docs', 'spike', 'init'];
    filtered = filtered.filter((task) => !SUPPORT_TASK_TYPES.includes(task.type));
  }

  if (filters.search) {
    const searchLower = filters.search.toLowerCase();
    filtered = filtered.filter(
      (task) =>
        task.title.toLowerCase().includes(searchLower) ||
        task.description?.toLowerCase().includes(searchLower) ||
        task.type?.toLowerCase().includes(searchLower) ||
        task.projectName?.toLowerCase().includes(searchLower) ||
        task.id.toLowerCase().startsWith(searchLower)
    );
  }

  const createdDateFilter = filters.createdDate || 'all';
  if (createdDateFilter !== 'all') {
    const now = new Date();
    const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const startOfWeek = new Date(startOfToday);
    const dayOfWeek = startOfWeek.getDay(); // 0 = Sunday, 1 = Monday...
    const weekOffset = dayOfWeek === 0 ? 6 : dayOfWeek - 1; // Monday as week start
    startOfWeek.setDate(startOfWeek.getDate() - weekOffset);
    const startOfMonth = new Date(now.getFullYear(), now.getMonth(), 1);
    const startOfLast30Days = new Date(now);
    startOfLast30Days.setDate(startOfLast30Days.getDate() - 30);

    filtered = filtered.filter((task) => {
      const createdAt = new Date(task.createdAt);
      if (Number.isNaN(createdAt.getTime())) {
        return false;
      }

      switch (createdDateFilter) {
        case 'today':
          return createdAt >= startOfToday;
        case 'this_week':
          return createdAt >= startOfWeek;
        case 'this_month':
          return createdAt >= startOfMonth;
        case 'last_30_days':
          return createdAt >= startOfLast30Days;
        default:
          return true;
      }
    });
  }

  return filtered;
}

/**
 * Generate initials from name
 */
function generateInitials(name: string): string {
  const parts = name.trim().split(/\s+/);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
  return name.substring(0, 2).toUpperCase();
}

/**
 * Map frontend status to backend status
 */
export function mapFrontendStatusToBackend(status: KanbanTask['status']): string {
  const reverseMap: Record<KanbanTask['status'], string> = {
    todo: 'todo',
    in_progress: 'in_progress',
    in_review: 'in_review',
    done: 'done',
    archived: 'archived',
  };
  return reverseMap[status];
}
