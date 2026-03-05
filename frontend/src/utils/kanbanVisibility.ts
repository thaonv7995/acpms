interface TaskLikeForKanbanVisibility {
  title?: unknown;
  metadata?: unknown;
}

/**
 * Breakdown AI analysis-session tasks are support-only tasks for requirement workflow,
 * so they should not appear on Kanban.
 */
export function isBreakdownSupportTask(task: TaskLikeForKanbanVisibility): boolean {
  const title = typeof task.title === 'string' ? task.title.trim() : '';
  const metadata =
    task.metadata && typeof task.metadata === 'object'
      ? (task.metadata as Record<string, unknown>)
      : {};

  const mode = String(metadata.breakdown_mode ?? '').trim().toLowerCase();
  const kind = String(metadata.breakdown_kind ?? '').trim().toLowerCase();

  return (
    mode === 'ai_support' ||
    kind === 'analysis_session' ||
    title.toLowerCase().startsWith('[breakdown]')
  );
}

