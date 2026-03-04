import { useMemo } from 'react';
import type {
  TimelineEntry,
  ToolCallEntry,
  SubagentEntry,
  SubagentThread,
} from '@/types/timeline-log';

/**
 * Detects subagent spawns from Task tool calls and creates nested timeline entries.
 *
 * This hook identifies when the Task tool is used (which spawns a subagent)
 * and creates SubagentEntry objects for the timeline display.
 */

export function useSubagentDetection(entries: TimelineEntry[]): {
  entries: TimelineEntry[];
  subagents: Map<string, SubagentThread>;
} {
  return useMemo(() => {
    const result: TimelineEntry[] = [];
    const subagentMap = new Map<string, SubagentThread>();

    for (const entry of entries) {
      // Check if this is a Task tool spawn
      if (entry.type === 'tool_call') {
        const toolEntry = entry as ToolCallEntry;

        if (toolEntry.toolName === 'Task' || toolEntry.actionType.action === 'task_create') {
          // Extract task description from actionType
          const taskDescription =
            (toolEntry.actionType as any).description ||
            (toolEntry.actionType as any).task ||
            'Subagent task';

          // Generate subagent ID (in real implementation, this would come from backend)
          const subagentId = `subagent-${entry.id}`;

          // Create subagent thread
          const thread: SubagentThread = {
            id: subagentId,
            parentAttemptId: 'current', // Would be actual attempt ID
            agentName: 'Subagent',
            taskDescription,
            status: toolEntry.status === 'running'
              ? 'running'
              : toolEntry.status === 'failed'
              ? 'failed'
              : 'completed',
            depth: 1, // Would calculate from parent
            entries: [], // Would be populated from subagent logs
            startedAt: entry.timestamp,
            completedAt: toolEntry.status !== 'running' ? entry.timestamp : undefined,
          };

          subagentMap.set(subagentId, thread);

          // Create subagent entry
          const subagentEntry: SubagentEntry = {
            id: entry.id,
            type: 'subagent',
            timestamp: entry.timestamp,
            thread,
          };

          result.push(subagentEntry);
        } else {
          result.push(entry);
        }
      } else {
        result.push(entry);
      }
    }

    return {
      entries: result,
      subagents: subagentMap,
    };
  }, [entries]);
}
