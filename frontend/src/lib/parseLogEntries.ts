/**
 * R5: Single transform layer - Raw logs → TimelineEntry[].
 * Consolidates normalizeLogToEntry + combineTextFragments into one parse flow.
 */
import type { TimelineEntry } from '@/types/timeline-log';
import { normalizeLogToEntry, type AgentLogLike } from './normalizeLogToEntry';
import { combineTextFragments } from './timeline-fragments';

/**
 * Parse raw agent logs into timeline entries.
 * Flow: normalize (per log) → flatten → combine text fragments.
 */
export function parseLogEntries(rawLogs: AgentLogLike[]): TimelineEntry[] {
  const entries = rawLogs.flatMap((log, index) => normalizeLogToEntry(log, index));
  return combineTextFragments(entries);
}
