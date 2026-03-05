/**
 * Hide internal/system prefixes from Kanban card titles.
 * We keep original task title in data layer and only clean display text.
 */
export function getKanbanDisplayTitle(title: string | undefined | null): string {
  const original = typeof title === 'string' ? title : '';
  const trimmed = original.trim();
  if (!trimmed) return '';

  // Examples:
  // - "[Breakdown][AI] Build login flow"
  // - "[Breakdown] [AI] Build login flow"
  // - "[AI][Breakdown] Build login flow"
  const cleaned = trimmed.replace(/^(?:\[(?:breakdown|ai)\]\s*)+/i, '').trim();
  return cleaned || trimmed;
}

