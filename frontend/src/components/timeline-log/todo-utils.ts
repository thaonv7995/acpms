export type TodoStatus =
  | 'pending'
  | 'in_progress'
  | 'completed'
  | 'cancelled'
  | 'unknown';

export interface TodoSummaryItem {
  content: string;
  status: TodoStatus;
}

function getTodoStatus(value: unknown): TodoStatus {
  const normalized = String(value || '')
    .replace(/\s+/g, '_')
    .replace(/-/g, '_')
    .toLowerCase();

  if (normalized === 'completed' || normalized === 'done') return 'completed';
  if (normalized === 'cancelled' || normalized === 'canceled') return 'cancelled';
  if (
    normalized === 'in_progress' ||
    normalized === 'inprogress' ||
    normalized === 'running' ||
    normalized === 'active'
  ) {
    return 'in_progress';
  }
  if (normalized === 'pending' || normalized === 'todo') return 'pending';
  return 'unknown';
}

export function parseTodoItems(
  source: unknown,
  sanitize?: (value: string) => string
): TodoSummaryItem[] {
  const rawItems = extractTodoArray(source);
  if (!rawItems) return [];

  return rawItems
    .map((item) => {
      if (!item || typeof item !== 'object') return null;
      const content =
        String((item as { content?: unknown }).content || '').trim() ||
        String((item as { active_form?: unknown }).active_form || '').trim() ||
        String((item as { activeForm?: unknown }).activeForm || '').trim();
      if (!content) return null;
      return {
        content: sanitize ? sanitize(content) : content,
        status: getTodoStatus((item as { status?: unknown }).status),
      };
    })
    .filter((item): item is TodoSummaryItem => item !== null);
}

function extractTodoArray(source: unknown): unknown[] | null {
  if (Array.isArray(source)) return source;
  if (!source || typeof source !== 'object') return null;

  const todos = (source as { todos?: unknown }).todos;
  if (Array.isArray(todos)) return todos;

  const argumentsTodos = (source as { arguments?: { todos?: unknown } }).arguments?.todos;
  if (Array.isArray(argumentsTodos)) return argumentsTodos;

  return null;
}
