import { useMemo } from 'react';
import type { TodoItem } from '@/bindings/TodoItem';
import type { NormalizedEntry } from '@/bindings/NormalizedEntry';
import type { PatchTypeWithKey } from '@/hooks/useConversationHistory';

interface UsePinnedTodosResult {
  todos: TodoItem[];
  lastUpdated: string | null;
}

/**
 * Hook that extracts and maintains the latest TODO state from normalized conversation entries.
 * Filters for TodoManagement ActionType entries and returns the most recent todo list.
 */
export const usePinnedTodos = (
  entries: PatchTypeWithKey[] | NormalizedEntry[]
): UsePinnedTodosResult => {
  return useMemo(() => {
    let latestTodos: TodoItem[] = [];
    let lastUpdatedTime: string | null = null;

    // Ensure entries is an array
    if (!entries || !Array.isArray(entries)) {
      return { todos: [], lastUpdated: null };
    }

    for (const entry of entries) {
      const normalizedEntry =
        'type' in entry && entry.type === 'NORMALIZED_ENTRY' && entry.content
          ? (entry.content as NormalizedEntry)
          : 'entry_type' in entry
            ? (entry as NormalizedEntry)
            : null;

      if (
        !normalizedEntry ||
        normalizedEntry.entry_type?.type !== 'tool_use' ||
        normalizedEntry.entry_type?.action_type?.action !== 'todo_management'
      ) {
        continue;
      }

      const actionType = normalizedEntry.entry_type.action_type;
      const partialTodos = actionType.todos || [];
      const currentTimestamp =
        normalizedEntry.timestamp || new Date().toISOString();

      const hasMeaningfulTodos =
        partialTodos.length > 0 &&
        partialTodos.every(
          (todo: TodoItem) =>
            todo.content && todo.content.trim().length > 0 && todo.status
        );
      const isNewerThanLatest =
        !lastUpdatedTime || currentTimestamp >= lastUpdatedTime;

      if (hasMeaningfulTodos && isNewerThanLatest) {
        latestTodos = partialTodos;
        lastUpdatedTime = currentTimestamp;
      }
    }

    return {
      todos: latestTodos,
      lastUpdated: lastUpdatedTime,
    };
  }, [entries]);
};
