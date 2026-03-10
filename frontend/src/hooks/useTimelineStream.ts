import { useMemo, useState } from 'react';
import { useQueries, useQuery } from '@tanstack/react-query';
import {
  useAttemptStream,
  type AttemptStreamConnectionState,
} from './useAttemptStream';
import { useExecutionProcessesStream } from './useExecutionProcessesStream';
import { useExecutionProcessLogsStream } from './useExecutionProcessLogsStream';
import { useOperationGrouping } from './useOperationGrouping';
import { useSubagentDetection } from './useSubagentDetection';
import { useAttemptDiffs } from './useAttemptDiffs';
import { getExecutionProcessNormalizedLogs, type ExecutionProcess } from '@/api/executionProcesses';
import { getAttemptLogs, type AgentLog } from '@/api/taskAttempts';
import type {
  TimelineEntry,
  ToolCallEntry,
  FileDiffSummary,
  TimelineTokenUsageInfo,
} from '@/types/timeline-log';

/**
 * Main hook for timeline display that combines streaming, grouping, and subagent detection.
 *
 * Flow:
 * 1. Stream raw logs from SSE
 * 2. Parse logs into timeline entries
 * 3. Detect and create subagent entries
 * 4. Group consecutive operations
 * 5. Return processed entries with streaming state
 */

interface UseTimelineStreamOptions {
  attemptId: string | undefined;
  /** When provided (e.g. from TaskAttemptPanel), avoids duplicate execution-processes stream subscription */
  executionProcesses?: ExecutionProcess[];
  enableGrouping?: boolean;
  enableSubagentDetection?: boolean;
  enableAutoScroll?: boolean;
}

interface UseTimelineStreamResult {
  entries: TimelineEntry[];
  tokenUsageInfo: TimelineTokenUsageInfo | null;
  isStreaming: boolean;
  isLoading: boolean;
  error: string | null;
  attemptStatus: string | null;
  streamState: AttemptStreamConnectionState;
  lastEventAt: number | null;
  autoScroll: boolean;
  setAutoScroll: (enabled: boolean) => void;
  hasMoreOlder: boolean;
  isLoadingOlder: boolean;
  loadOlder: () => Promise<void>;
  reconnect: () => void;
}

function sortAgentLogs(logs: AgentLog[]): AgentLog[] {
  return [...logs].sort((a, b) => {
    const ta = Date.parse(a.created_at);
    const tb = Date.parse(b.created_at);
    if (ta !== tb) return ta - tb;
    return a.id.localeCompare(b.id);
  });
}

function getAgentLogKey(log: AgentLog): string {
  return `${log.id}|${log.created_at}|${log.log_type}|${log.content}`;
}

function mergeAgentLogArrays(logArrays: AgentLog[][]): AgentLog[] {
  const merged: AgentLog[] = [];
  const seen = new Set<string>();

  for (const logs of logArrays) {
    for (const log of sortAgentLogs(logs)) {
      const key = getAgentLogKey(log);
      if (seen.has(key)) continue;
      seen.add(key);
      merged.push(log);
    }
  }

  return sortAgentLogs(merged);
}

function normalizeComparablePath(path: string): string {
  return path
    .replace(/\\/g, '/')
    .replace(/^\.\/+/, '')
    .replace(/^(?:a|b)\//, '')
    .replace(/^\/+/, '')
    .trim();
}

function pathBaseName(path: string): string {
  const normalized = normalizeComparablePath(path);
  const index = normalized.lastIndexOf('/');
  return index >= 0 ? normalized.slice(index + 1) : normalized;
}

export function resolveFileDiffForPath(path: string | undefined, diffs: FileDiffSummary[]): FileDiffSummary | null {
  if (!path || diffs.length === 0) return null;

  const normalizedPath = normalizeComparablePath(path);
  if (!normalizedPath) return null;

  const exact = diffs.filter((diff) => normalizeComparablePath(diff.file_path) === normalizedPath);
  if (exact.length === 1) return exact[0];
  if (exact.length > 1) return exact[0];

  const suffixMatches = diffs.filter((diff) => {
    const diffPath = normalizeComparablePath(diff.file_path);
    return diffPath.endsWith(`/${normalizedPath}`) || normalizedPath.endsWith(`/${diffPath}`);
  });
  if (suffixMatches.length === 1) return suffixMatches[0];

  if (suffixMatches.length > 1) {
    const baseName = pathBaseName(normalizedPath);
    const basenameMatches = suffixMatches.filter(
      (diff) => pathBaseName(diff.file_path) === baseName
    );
    if (basenameMatches.length === 1) return basenameMatches[0];
  }

  return null;
}

export function isAttemptUserInputLog(log: AgentLog): boolean {
  const logType = log.log_type?.toLowerCase();
  return logType === 'user' || logType === 'stdin';
}

export function mergeExecutionProcessLogsWithAttemptUserInputs(
  executionProcessLogs: AgentLog[],
  attemptLogs: AgentLog[]
): AgentLog[] {
  const userInputLogs = attemptLogs.filter(isAttemptUserInputLog);
  if (userInputLogs.length === 0) {
    return executionProcessLogs;
  }

  return mergeAgentLogArrays([executionProcessLogs, userInputLogs]);
}

export function mergeExecutionProcessLogs(
  processIdsInOrder: string[],
  historicalProcessLogsById: Map<string, AgentLog[]>,
  latestProcessId: string | undefined,
  latestProcessLogs: AgentLog[]
): AgentLog[] {
  if (processIdsInOrder.length === 0) return [];

  const merged: AgentLog[] = [];
  const seen = new Set<string>();

  for (const processId of processIdsInOrder) {
    const processLogs = processId === latestProcessId
      ? latestProcessLogs
      : (historicalProcessLogsById.get(processId) || []);

    for (const log of sortAgentLogs(processLogs)) {
      const key = getAgentLogKey(log);
      if (seen.has(key)) continue;
      seen.add(key);
      merged.push(log);
    }
  }

  return sortAgentLogs(merged);
}

import { parseLogEntries } from '@/lib/parseLogEntries';
import type { AgentLogLike } from '@/lib/normalizeLogToEntry';

/**
 * Re-export for consumers that need combineTextFragments directly.
 * @deprecated Prefer parseLogEntries from @/lib/parseLogEntries for raw→TimelineEntry flow.
 */
export { combineTextFragments } from '@/lib/timeline-fragments';

/**
 * Enrich timeline entries with file diff statistics.
 * Matches file paths from tool calls to file diffs from the backend.
 */
function enrichWithDiffs(entries: TimelineEntry[], diffs: FileDiffSummary[]): TimelineEntry[] {
  if (diffs.length === 0) return entries;

  return entries.map(entry => {
    if (entry.type === 'tool_call') {
      const toolCall = entry as ToolCallEntry;
      const filePath = toolCall.actionType.path || toolCall.actionType.file_path;

      if (filePath) {
        const diff = resolveFileDiffForPath(filePath, diffs);
        if (diff) {
          return {
            ...toolCall,
            diffStats: {
              additions: diff.additions,
              deletions: diff.deletions,
            },
            diffId: diff.id,
          };
        }
      }
    }

    if (entry.type === 'file_change') {
      const fileEntry = entry as TimelineEntry & { path?: string; diffId?: string };
      const filePath = fileEntry.path;
      if (filePath) {
        const diff = resolveFileDiffForPath(filePath, diffs);
        if (diff) {
          const linesAdded =
            typeof fileEntry.linesAdded === 'number'
              ? fileEntry.linesAdded
              : diff.additions;
          const linesRemoved =
            typeof fileEntry.linesRemoved === 'number'
              ? fileEntry.linesRemoved
              : diff.deletions;

          return {
            ...fileEntry,
            diffId: diff.id,
            linesAdded,
            linesRemoved,
          };
        }
      }
    }

    return entry;
  });
}

export function extractTokenUsageInfo(log: AgentLog): TimelineTokenUsageInfo | null {
  if (log.log_type?.toLowerCase() !== 'normalized') {
    return null;
  }

  try {
    const parsed = JSON.parse(log.content);
    const entryType = parsed?.entry_type;
    if (entryType?.type !== 'token_usage_info') {
      return null;
    }

    const inputTokens = Number(entryType.input_tokens ?? 0);
    const outputTokens = Number(entryType.output_tokens ?? 0);
    const totalTokens = entryType.total_tokens != null
      ? Number(entryType.total_tokens)
      : inputTokens + outputTokens;
    const modelContextWindow = entryType.model_context_window != null
      ? Number(entryType.model_context_window)
      : undefined;

    if (!Number.isFinite(inputTokens) || !Number.isFinite(outputTokens) || !Number.isFinite(totalTokens)) {
      return null;
    }

    if (inputTokens <= 0 && outputTokens <= 0 && totalTokens <= 0) {
      return null;
    }

    return {
      inputTokens: Math.max(0, Math.round(inputTokens)),
      outputTokens: Math.max(0, Math.round(outputTokens)),
      totalTokens: Math.max(0, Math.round(totalTokens)),
      modelContextWindow:
        modelContextWindow != null && Number.isFinite(modelContextWindow) && modelContextWindow > 0
          ? Math.round(modelContextWindow)
          : undefined,
    };
  } catch {
    return null;
  }
}

export function extractLatestTokenUsageInfo(logs: AgentLog[]): TimelineTokenUsageInfo | null {
  for (let i = logs.length - 1; i >= 0; i -= 1) {
    const info = extractTokenUsageInfo(logs[i]);
    if (info) return info;
  }
  return null;
}

export function useTimelineStream(
  options: UseTimelineStreamOptions
): UseTimelineStreamResult {
  const {
    attemptId,
    executionProcesses: executionProcessesProp,
    enableGrouping = true,
    enableSubagentDetection = true,
    enableAutoScroll = true,
  } = options;

  const [autoScroll, setAutoScroll] = useState(enableAutoScroll);

  const { processes: executionProcessesFromHook } = useExecutionProcessesStream(
    executionProcessesProp ? undefined : attemptId
  );
  const executionProcesses = executionProcessesProp ?? executionProcessesFromHook;

  // Stream raw logs (useAttemptStream should handle undefined)
  const {
    logs: attemptLogs,
    attempt,
    isConnected,
    isLoading,
    error,
    hasMoreOlder,
    isLoadingOlder,
    loadOlder,
    reconnect: reconnectAttemptStream,
    connectionState,
    lastEventAt,
  } = useAttemptStream(attemptId);

  // Process-aware stream path (remote parity direction):
  // combine historical execution-process snapshots + latest process stream.
  const executionProcessIds = useMemo(
    () => executionProcesses.map((process) => process.id),
    [executionProcesses]
  );
  const latestExecutionProcessId = useMemo(
    () => (
      executionProcessIds.length > 0
        ? executionProcessIds[executionProcessIds.length - 1]
        : undefined
    ),
    [executionProcessIds]
  );
  const historicalExecutionProcessIds = useMemo(
    () => executionProcessIds.filter((processId) => processId !== latestExecutionProcessId),
    [executionProcessIds, latestExecutionProcessId]
  );
  const attemptHistoryQuery = useQuery({
    queryKey: ['attempt-logs-full', attemptId],
    queryFn: () => getAttemptLogs(attemptId!),
    enabled: Boolean(attemptId),
    staleTime: 30_000,
  });
  // Defer historical process logs until we know attempt logs are empty (avoids N+1 burst on mount).
  // Process logs are only used as fallback when attempt logs (HTTP + stream) have nothing.
  const attemptLogsKnownEmpty =
    attemptHistoryQuery.isSuccess && (attemptHistoryQuery.data?.length ?? 0) === 0;
  const historicalProcessLogsQueries = useQueries({
    queries: historicalExecutionProcessIds.map((processId) => ({
      queryKey: ['execution-process-logs', processId, 'normalized'],
      queryFn: () => getExecutionProcessNormalizedLogs(processId),
      staleTime: 30_000,
      enabled: Boolean(processId) && attemptLogsKnownEmpty,
    })),
  });
  const {
    logs: processLogs,
    isLoading: isProcessLogsLoading,
    isStreaming: isProcessStreaming,
    error: processStreamError,
    attemptStatus: processAttemptStatus,
    reconnect: reconnectProcessLogs,
  } = useExecutionProcessLogsStream(latestExecutionProcessId, 'normalized');
  const historicalProcessLogsById = useMemo(() => {
    const logsByProcess = new Map<string, AgentLog[]>();
    historicalExecutionProcessIds.forEach((processId, index) => {
      logsByProcess.set(processId, historicalProcessLogsQueries[index]?.data || []);
    });
    return logsByProcess;
  }, [historicalExecutionProcessIds, historicalProcessLogsQueries]);
  const liveAttemptLogs = useMemo<AgentLog[]>(
    () =>
      attemptLogs
        .flatMap((log) => {
          if (typeof log.id !== 'string') return [];
          const createdAt = log.created_at ?? log.timestamp;
          if (typeof createdAt !== 'string') return [];
          return [{
            id: log.id,
            attempt_id: log.attempt_id,
            log_type: log.log_type,
            content: log.content,
            created_at: createdAt,
          }];
        }),
    [attemptLogs]
  );
  const historicalAttemptLogs = attemptHistoryQuery.data || [];
  const useProcessLogs = executionProcessIds.length > 0;
  const logs = useMemo(() => {
    const mergedAttemptLogs = mergeAgentLogArrays([historicalAttemptLogs, liveAttemptLogs]);

    // Defensive: filter out any logs that don't belong to this attempt (prevents cross-contamination
    // when multiple TimelineLogDisplay instances are mounted, e.g. Agent Stream page with 2+ running sessions)
    const scopedAttemptLogs =
      attemptId && mergedAttemptLogs.length > 0
        ? mergedAttemptLogs.filter((log) => String(log.attempt_id) === String(attemptId))
        : mergedAttemptLogs;

    // Prefer full attempt logs (system, stdout, stderr, etc.) so completed view matches running view.
    // Process logs (normalized) only contain log_type=normalized - a curated subset that hides raw output.
    if (scopedAttemptLogs.length > 0) {
      return scopedAttemptLogs;
    }

    // Fallback: when attempt logs are empty, use process logs + user inputs
    if (!useProcessLogs) {
      return scopedAttemptLogs;
    }

    const mergedProcessLogs = mergeExecutionProcessLogs(
      executionProcessIds,
      historicalProcessLogsById,
      latestExecutionProcessId,
      processLogs
    );

    return mergeExecutionProcessLogsWithAttemptUserInputs(mergedProcessLogs, scopedAttemptLogs);
  }, [
    attemptId,
    historicalAttemptLogs,
    liveAttemptLogs,
    useProcessLogs,
    executionProcessIds,
    historicalProcessLogsById,
    latestExecutionProcessId,
    processLogs,
  ]);
  const isHistoricalProcessLogsLoading = historicalProcessLogsQueries.some((query) => query.isLoading);
  const historicalProcessStreamError = historicalProcessLogsQueries
    .map((query) => (query.error instanceof Error ? query.error.message : null))
    .find((message): message is string => Boolean(message)) || null;
  const attemptStatusHint = processAttemptStatus ?? attempt?.status?.toLowerCase();
  const shouldFetchTimelineDiffMetadata =
    attemptStatusHint === 'success' ||
    attemptStatusHint === 'failed' ||
    attemptStatusHint === 'cancelled';

  // Fetch file diffs for this attempt
  const { diffs } = useAttemptDiffs(attemptId, {
    enabled: shouldFetchTimelineDiffMetadata,
  });

  // Return early if no attemptId
  const hasAttemptId = !!attemptId;

  // Parse logs into timeline entries
  const parsedEntries = useMemo(() => {
    if (!hasAttemptId) return [];

    const combined = parseLogEntries(logs as AgentLogLike[]);

    // Filter out incomplete tool entries (status = "created")
    const filtered = combined.filter(entry => {
      if (entry.type === 'tool_call') {
        // Only show completed tools (success/failed), hide "created" status
        return entry.status !== 'created';
      }
      return true; // Keep all other entry types
    });

    return filtered;
  }, [logs, hasAttemptId]);

  const tokenUsageInfo = useMemo(
    () => extractLatestTokenUsageInfo(logs),
    [logs]
  );

  // Detect subagents
  const { entries: entriesWithSubagentsDetected } = useSubagentDetection(parsedEntries);
  const entriesWithSubagents = enableSubagentDetection
    ? entriesWithSubagentsDetected
    : parsedEntries;

  // Group operations
  const groupedEntries = useOperationGrouping(entriesWithSubagents);
  const groupAdjustedEntries = enableGrouping ? groupedEntries : entriesWithSubagents;

  const attemptStatus = attemptStatusHint;
  const attemptHistoryError =
    attemptHistoryQuery.error instanceof Error ? attemptHistoryQuery.error.message : null;

  // Enrich with file diff statistics
  const finalEntries = useMemo(() => {
    const enriched = enrichWithDiffs(groupAdjustedEntries, diffs);

    // If the attempt is no longer active, don't leave "running" tool cards hanging.
    // (Some providers only emit tool start lines and a terminal attempt status.)
    if (attemptStatus === 'success' || attemptStatus === 'failed' || attemptStatus === 'cancelled') {
      const terminalStatus =
        attemptStatus === 'success'
          ? 'success'
          : attemptStatus === 'failed'
            ? 'failed'
            : 'cancelled';

      return enriched.map((entry) => {
        if (entry.type !== 'tool_call') return entry;
        if (entry.status !== 'running') return entry;
        return { ...entry, status: terminalStatus };
      });
    }

    return enriched;
  }, [groupAdjustedEntries, diffs, attemptStatus]);
  const isAttemptActive = attemptStatus === 'running' || attemptStatus === 'queued';
  const useProcessStreamState = useProcessLogs;
  const streamState = hasAttemptId ? connectionState : 'idle';
  const combinedError = !hasAttemptId
    ? null
    : (processStreamError || historicalProcessStreamError || attemptHistoryError || error);

  const reconnectTimeline = () => {
    reconnectAttemptStream();
    void attemptHistoryQuery.refetch();
    if (useProcessStreamState) {
      reconnectProcessLogs();
      historicalProcessLogsQueries.forEach((query) => {
        void query.refetch();
      });
    }
  };

  return {
    entries: finalEntries,
    tokenUsageInfo,
    isStreaming:
      hasAttemptId &&
      (useProcessStreamState
        ? isProcessStreaming
        : (streamState === 'live' ||
          streamState === 'stale' ||
          streamState === 'reconnecting' ||
          (isConnected && (!attemptStatus || isAttemptActive)))),
    // When we already have logs to display, don't show loading for historical process logs
    // (avoids jarring reload when follow-up adds new process and historical query loads)
    isLoading: hasAttemptId
      ? (
          isLoading ||
          isProcessLogsLoading ||
          (logs.length === 0 ? isHistoricalProcessLogsLoading : false) ||
          attemptHistoryQuery.isLoading
        )
      : false,
    error: combinedError,
    attemptStatus: attemptStatus ?? null,
    streamState,
    lastEventAt,
    autoScroll,
    setAutoScroll,
    hasMoreOlder,
    isLoadingOlder,
    loadOlder,
    reconnect: reconnectTimeline,
  };
}
