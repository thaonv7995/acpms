import { useMemo } from 'react';
import type { TimelineEntry, ToolCallEntry, OperationGroup } from '@/types/timeline-log';

/**
 * Groups consecutive operations of the same type into operation groups
 * for cleaner timeline display.
 *
 * Grouping rules:
 * - Must have 3+ consecutive operations of the same type
 * - Operations must be within 5 seconds of each other
 * - Supported types: file_read, search, file_edit
 */

const GROUPABLE_ACTIONS = ['file_read', 'search', 'file_edit'] as const;
const MIN_GROUP_SIZE = 3;
const MAX_TIME_GAP_MS = 5000; // 5 seconds

interface GroupBuffer {
  type: string;
  entries: ToolCallEntry[];
  startTime: Date;
  lastTime: Date;
}

export function useOperationGrouping(entries: TimelineEntry[]): TimelineEntry[] {
  return useMemo(() => {
    const result: TimelineEntry[] = [];
    let buffer: GroupBuffer | null = null;

    const flushBuffer = () => {
      if (!buffer) return;

      if (buffer.entries.length >= MIN_GROUP_SIZE) {
        // Create operation group
        const group: OperationGroup = {
          id: `group-${buffer.entries[0].id}`,
          type: 'operation_group',
          groupType: buffer.type as any,
          operations: buffer.entries,
          count: buffer.entries.length,
          timestamp: buffer.entries[0].timestamp,
          timestamp_start: buffer.entries[0].timestamp,
          timestamp_end: buffer.entries[buffer.entries.length - 1].timestamp,
          status: buffer.entries.some(e => e.status === 'failed')
            ? 'failed'
            : buffer.entries.some(e => e.status === 'running')
            ? 'running'
            : 'success',
        };
        result.push(group);
      } else {
        // Too few entries, add individually
        result.push(...buffer.entries);
      }

      buffer = null;
    };

    for (const entry of entries) {
      // Only group tool_call entries
      if (entry.type !== 'tool_call') {
        flushBuffer();
        result.push(entry);
        continue;
      }

      const toolEntry = entry as ToolCallEntry;
      const actionType = toolEntry.actionType.action;

      // Check if this action is groupable
      if (!GROUPABLE_ACTIONS.includes(actionType as any)) {
        flushBuffer();
        result.push(entry);
        continue;
      }

      const entryTime = new Date(entry.timestamp);

      // Start new buffer or add to existing
      if (!buffer || buffer.type !== actionType) {
        flushBuffer();
        buffer = {
          type: actionType,
          entries: [toolEntry],
          startTime: entryTime,
          lastTime: entryTime,
        };
      } else {
        // Check time gap
        const timeDiff = entryTime.getTime() - buffer.lastTime.getTime();

        if (timeDiff > MAX_TIME_GAP_MS) {
          // Gap too large, flush and start new buffer
          flushBuffer();
          buffer = {
            type: actionType,
            entries: [toolEntry],
            startTime: entryTime,
            lastTime: entryTime,
          };
        } else {
          // Add to buffer
          buffer.entries.push(toolEntry);
          buffer.lastTime = entryTime;
        }
      }
    }

    // Flush remaining buffer
    flushBuffer();

    return result;
  }, [entries]);
}
