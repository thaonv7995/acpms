import type { ProjectLifecycleStatus, ProjectSummary } from '../types/repository';

type ProjectStatusColor = 'yellow' | 'blue' | 'emerald' | 'green' | 'slate' | 'red';

interface ProjectStatusPresentation {
  status: ProjectLifecycleStatus;
  statusLabel: string;
  statusColor: ProjectStatusColor;
  progress: number;
  agentCount: number;
}

const STATUS_PRESENTATION: Record<
  ProjectLifecycleStatus,
  { label: string; color: ProjectStatusColor }
> = {
  planning: { label: 'Planning', color: 'slate' },
  active: { label: 'Active', color: 'blue' },
  reviewing: { label: 'Reviewing', color: 'yellow' },
  blocked: { label: 'Blocked', color: 'red' },
  completed: { label: 'Completed', color: 'green' },
  paused: { label: 'Paused', color: 'slate' },
  archived: { label: 'Archived', color: 'slate' },
};

export function normalizeProjectLifecycleStatus(value: unknown): ProjectLifecycleStatus | null {
  if (typeof value !== 'string') {
    return null;
  }

  const normalized = value.trim().toLowerCase().replace(/[\s-]+/g, '_');
  if (
    normalized === 'planning' ||
    normalized === 'active' ||
    normalized === 'reviewing' ||
    normalized === 'blocked' ||
    normalized === 'completed' ||
    normalized === 'paused' ||
    normalized === 'archived'
  ) {
    return normalized;
  }

  return null;
}

export function getProjectStatusPresentation(
  summary?: ProjectSummary | null,
): ProjectStatusPresentation {
  const normalizedStatus = normalizeProjectLifecycleStatus(summary?.lifecycle_status) || 'planning';
  const fallbackPresentation = STATUS_PRESENTATION[normalizedStatus];
  const progressValue = typeof summary?.progress === 'number' ? summary.progress : 0;
  const agentCount = typeof summary?.active_tasks === 'number' ? summary.active_tasks : 0;

  return {
    status: normalizedStatus,
    statusLabel: fallbackPresentation.label,
    statusColor: fallbackPresentation.color,
    progress: Math.max(0, Math.min(100, Math.round(progressValue))),
    agentCount: Math.max(0, Math.round(agentCount)),
  };
}
