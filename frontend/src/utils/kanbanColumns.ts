import type { KanbanColumn, KanbanTask } from '../types/project';

export interface KanbanColumnConfig {
  showBacklog: boolean;
  showClosed: boolean;
}

type ToggleKey = keyof KanbanColumnConfig;

function parseBooleanLike(value: unknown): boolean {
  if (typeof value === 'boolean') return value;
  const normalized = String(value ?? '')
    .trim()
    .toLowerCase();
  return normalized === '1' || normalized === 'true' || normalized === 'yes' || normalized === 'on';
}

function readEnvToggle(envKey: string): boolean {
  const env = import.meta.env as Record<string, unknown>;
  return parseBooleanLike(env[envKey]);
}

const ENV_DEFAULT_COLUMN_CONFIG: Readonly<KanbanColumnConfig> = {
  showBacklog: readEnvToggle('VITE_KANBAN_SHOW_BACKLOG'),
  showClosed: readEnvToggle('VITE_KANBAN_SHOW_CLOSED'),
};

export function getDefaultKanbanColumnConfig(): KanbanColumnConfig {
  return { ...ENV_DEFAULT_COLUMN_CONFIG };
}

export function normalizeKanbanColumnConfig(
  config?: Partial<KanbanColumnConfig>
): KanbanColumnConfig {
  const defaults = getDefaultKanbanColumnConfig();
  return {
    showBacklog: config?.showBacklog ?? defaults.showBacklog,
    showClosed: config?.showClosed ?? defaults.showClosed,
  };
}

const KANBAN_COLUMN_DEFS: Array<
  Omit<KanbanColumn, 'tasks'> & {
    toggleKey?: ToggleKey;
  }
> = [
  { id: 'col-backlog', title: 'BACKLOG', status: 'backlog', color: 'slate', toggleKey: 'showBacklog' },
  { id: 'col-todo', title: 'TO DO', status: 'todo', color: 'slate' },
  { id: 'col-in-progress', title: 'AGENT PROCESSING', status: 'in_progress', color: 'blue' },
  { id: 'col-in-review', title: 'IN REVIEW', status: 'in_review', color: 'yellow' },
  { id: 'col-done', title: 'COMPLETED', status: 'done', color: 'green' },
  { id: 'col-closed', title: 'CLOSED', status: 'archived', color: 'zinc', toggleKey: 'showClosed' },
];

export function createKanbanColumns(columnConfig?: Partial<KanbanColumnConfig>): KanbanColumn[] {
  const config = normalizeKanbanColumnConfig(columnConfig);
  return KANBAN_COLUMN_DEFS.filter((column) => {
    if (!column.toggleKey) return true;
    return config[column.toggleKey];
  }).map((column) => ({
    id: column.id,
    title: column.title,
    status: column.status,
    color: column.color,
    tasks: [],
  }));
}

/**
 * Map task status to a visible Kanban status lane.
 * - blocked tasks are grouped into "in_review"
 * - closed column is optional and hidden by default
 */
export function resolveKanbanColumnStatus(
  status: KanbanTask['status'],
  columnConfig?: Partial<KanbanColumnConfig>
): KanbanTask['status'] | null {
  const config = normalizeKanbanColumnConfig(columnConfig);
  switch (status) {
    case 'backlog':
      // When backlog column is hidden, keep backlog tasks visible under TODO.
      return config.showBacklog ? 'backlog' : 'todo';
    case 'blocked':
      return 'in_review';
    case 'archived':
      return config.showClosed ? 'archived' : null;
    default:
      return status;
  }
}

export function resolveKanbanColumnId(
  task: Pick<KanbanTask, 'status'>,
  columnConfig?: Partial<KanbanColumnConfig>
): string | null {
  const config = normalizeKanbanColumnConfig(columnConfig);
  const status = resolveKanbanColumnStatus(task.status, config);
  if (!status) return null;

  if (status === 'backlog') return 'col-backlog';
  if (status === 'todo') {
    return 'col-todo';
  }
  if (status === 'archived') return 'col-closed';
  if (status === 'in_progress') return 'col-in-progress';
  if (status === 'in_review') return 'col-in-review';
  if (status === 'done') return 'col-done';

  return null;
}

export function isKanbanStatusVisible(
  status: KanbanTask['status'],
  columnConfig?: Partial<KanbanColumnConfig>
): boolean {
  return resolveKanbanColumnStatus(status, columnConfig) !== null;
}
