/**
 * R3: Utility to apply stream patch operations (extracted from useAttemptStream).
 * Handles attempt stream Patch messages: /logs/- add, /status replace.
 */

export interface LogEntryLike {
  id?: string;
  attempt_id?: string;
  log_type?: string;
  content?: string;
  timestamp?: string;
  created_at?: string;
  tool_name?: string | null;
}

export interface StreamPatchOperation {
  op: string;
  path: string;
  value: unknown;
}

/**
 * Apply a batch of log patches (add/merge) to previous logs.
 * Used when debouncing multiple /logs/- add operations.
 */
export function applyLogPatches<T extends LogEntryLike>(
  prev: T[],
  patches: T[]
): T[] {
  let next = prev;
  for (const incoming of patches) {
    const incomingId = incoming.id;
    if (incomingId) {
      const idx = next.findIndex((l) => l.id === incomingId);
      if (idx !== -1) {
        const existing = next[idx];
        const merged = { ...existing, ...incoming } as T;
        merged.created_at = (existing.created_at ?? incoming.created_at) as T['created_at'];
        merged.timestamp = (existing.timestamp ?? incoming.timestamp) as T['timestamp'];
        const arr = next.slice();
        arr[idx] = merged;
        next = arr;
        continue;
      }
    }
    next = [...next, incoming];
  }
  return next;
}

/**
 * Apply a single stream patch operation to logs.
 * Returns new logs array or null if operation doesn't affect logs.
 */
export function applyStreamPatchToLogs<T extends LogEntryLike>(
  prev: T[],
  operation: StreamPatchOperation
): T[] | null {
  if (operation.path === '/logs/-' && operation.op === 'add') {
    return applyLogPatches(prev, [operation.value as T]);
  }
  return null;
}

/**
 * Apply a single stream patch operation to attempt status.
 * Returns new status string or null if operation doesn't affect status.
 */
export function applyStreamPatchToAttemptStatus(
  _prevStatus: string | undefined,
  operation: StreamPatchOperation
): string | null {
  if (operation.path === '/status' && operation.op === 'replace') {
    return String(operation.value);
  }
  return null;
}
