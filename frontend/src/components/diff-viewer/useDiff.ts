/**
 * useDiff - Hook for fetching and managing diff data
 *
 * Features:
 * - Fetch diff + branch status from API
 * - Periodic refresh while active (Vibe-like experience)
 * - Loading/error states
 * - Retry capability
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { apiGet, API_PREFIX, ApiError } from '../../api/client';
import type { DiffFile, DiffSummary, BranchInfo, AvailableActions } from './types';
import { transformApiResponse, type ApiDiffResponse } from './diff-api-transform';

interface UseDiffOptions {
  attemptId?: string;
  enabled?: boolean;
  realtime?: boolean;
}

interface UseDiffState {
  files: Map<string, DiffFile>;
  summary: DiffSummary;
  branchInfo: BranchInfo | null;
  availableActions: AvailableActions | null;
  isComplete: boolean;
}

interface UseDiffReturn {
  files: DiffFile[];
  summary: DiffSummary;
  branchInfo: BranchInfo | null;
  availableActions: AvailableActions | null;
  isComplete: boolean;
  isLoading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

interface ApiBranchStatusResponse {
  branch_name: string;
  target_branch_name: string;
  ahead_count: number;
  behind_count: number;
  has_conflicts: boolean;
  is_attempt_active?: boolean;
  can_push: boolean;
  can_merge: boolean;
  pr_url?: string;
  pr_status?: string;
}

interface DiffCacheEntry {
  files: DiffFile[];
  summary: DiffSummary;
  branchInfo: BranchInfo | null;
  availableActions: AvailableActions | null;
  isComplete: boolean;
  cachedAt: number;
}

const DEFAULT_SUMMARY: DiffSummary = {
  totalFiles: 0,
  totalAdditions: 0,
  totalDeletions: 0,
  filesAdded: 0,
  filesModified: 0,
  filesDeleted: 0,
  filesRenamed: 0,
};

const DEFAULT_ACTIONS: AvailableActions = {
  canMerge: false,
  canCreatePR: false,
  canRebase: false,
  canReject: true,
};

const POLL_INTERVAL_MS = 4000;
const DIFF_CACHE_TTL_MS = 15000;
const DIFF_CACHE = new Map<string, DiffCacheEntry>();

/** Invalidate cache for an attempt so next fetch gets fresh branch-status (incl. MR sync from GitLab). */
export function invalidateDiffCache(attemptId: string | undefined): void {
  if (attemptId) DIFF_CACHE.delete(attemptId);
}

function mapBranchInfo(status: ApiBranchStatusResponse): BranchInfo {
  return {
    source: status.branch_name,
    target: status.target_branch_name,
    commitsAhead: status.ahead_count,
    commitsBehind: status.behind_count,
    hasConflicts: status.has_conflicts,
  };
}

function mapAvailableActions(status: ApiBranchStatusResponse, isAttemptActive: boolean): AvailableActions {
  // Keep actions disabled while attempt is still active.
  if (isAttemptActive) {
    return DEFAULT_ACTIONS;
  }
  const canCreatePR = Boolean(status.can_push);
  const canMerge = Boolean(status.can_merge && !status.has_conflicts);
  return {
    canMerge,
    canCreatePR,
    canRebase: Boolean(status.behind_count > 0 || status.has_conflicts),
    canReject: canMerge || canCreatePR,
  };
}

function getCachedDiff(attemptId: string | undefined): DiffCacheEntry | null {
  if (!attemptId) return null;
  const cached = DIFF_CACHE.get(attemptId);
  if (!cached) return null;
  if (Date.now() - cached.cachedAt > DIFF_CACHE_TTL_MS) {
    DIFF_CACHE.delete(attemptId);
    return null;
  }
  return cached;
}

function toState(entry: DiffCacheEntry): UseDiffState {
  const filesMap = new Map<string, DiffFile>();
  entry.files.forEach((file) => filesMap.set(file.path, file));
  return {
    files: filesMap,
    summary: entry.summary,
    branchInfo: entry.branchInfo,
    availableActions: entry.availableActions,
    isComplete: entry.isComplete,
  };
}

const FALLBACK_BRANCH_INFO: BranchInfo = {
  source: '',
  target: 'main',
  commitsAhead: 0,
  commitsBehind: 0,
  hasConflicts: false,
};

function buildDiffCacheEntry(
  files: DiffFile[],
  summary: DiffSummary,
  branchResult: PromiseSettledResult<ApiBranchStatusResponse> | null,
  fallbackIsComplete: boolean,
  attemptId: string,
  cachedAt: number
): DiffCacheEntry {
  const isAttemptActive =
    branchResult?.status === 'fulfilled'
      ? Boolean(branchResult.value.is_attempt_active)
      : !fallbackIsComplete;

  const branchInfo =
    branchResult?.status === 'fulfilled'
      ? mapBranchInfo(branchResult.value)
      : { ...FALLBACK_BRANCH_INFO, source: `attempt/${attemptId.slice(0, 8)}` };

  const availableActions =
    branchResult?.status === 'fulfilled'
      ? mapAvailableActions(branchResult.value, isAttemptActive)
      : DEFAULT_ACTIONS;

  return {
    files,
    summary,
    branchInfo,
    availableActions,
    isComplete: !isAttemptActive,
    cachedAt,
  };
}

/** Fetch diff only (fast, local). Branch-status (calls GitLab) is fetched separately. */
async function fetchDiffOnly(attemptId: string): Promise<{ files: DiffFile[]; summary: DiffSummary }> {
  const response = await apiGet<ApiDiffResponse>(`${API_PREFIX}/attempts/${attemptId}/diff`);
  const transformed = transformApiResponse(response);
  return { files: transformed.files, summary: transformed.summary };
}

/** Fetch branch-status only (can be slow - calls GitLab). */
async function fetchBranchStatusOnly(
  attemptId: string
): Promise<ApiBranchStatusResponse> {
  return apiGet<ApiBranchStatusResponse>(`${API_PREFIX}/attempts/${attemptId}/branch-status`);
}

export async function prefetchDiffData(attemptId: string): Promise<void> {
  const cached = getCachedDiff(attemptId);
  if (cached) return;
  try {
    const { files, summary } = await fetchDiffOnly(attemptId);
    const snapshot = buildDiffCacheEntry(
      files,
      summary,
      null,
      false,
      attemptId,
      Date.now()
    );
    DIFF_CACHE.set(attemptId, snapshot);
    // Prefetch branch-status in background for when user opens diff view
    fetchBranchStatusOnly(attemptId)
      .then((branchStatus) => {
        const fullSnapshot = buildDiffCacheEntry(
          files,
          summary,
          { status: 'fulfilled', value: branchStatus },
          false,
          attemptId,
          Date.now()
        );
        DIFF_CACHE.set(attemptId, fullSnapshot);
      })
      .catch(() => {});
  } catch {
    // Diff not ready yet (404) - ignore
  }
}

export function useDiff({ attemptId, enabled = true, realtime = false }: UseDiffOptions): UseDiffReturn {
  const initialCached = getCachedDiff(attemptId);
  const [state, setState] = useState<UseDiffState>(() =>
    initialCached
      ? toState(initialCached)
      : {
          files: new Map(),
          summary: DEFAULT_SUMMARY,
          branchInfo: null,
          availableActions: null,
          isComplete: false,
        }
  );
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasLoadedRef = useRef(Boolean(initialCached));
  const isCompleteRef = useRef(state.isComplete);

  useEffect(() => {
    isCompleteRef.current = state.isComplete;
  }, [state.isComplete]);

  useEffect(() => {
    const cached = getCachedDiff(attemptId);
    if (cached) {
      setState(toState(cached));
      hasLoadedRef.current = true;
      return;
    }

    setState({
      files: new Map(),
      summary: DEFAULT_SUMMARY,
      branchInfo: null,
      availableActions: null,
      isComplete: false,
    });
    hasLoadedRef.current = false;
    isCompleteRef.current = false;
  }, [attemptId]);

  const fetchDiff = useCallback(async (silent = false) => {
    if (!attemptId) return;

    const shouldShowBlockingLoader = !silent && !hasLoadedRef.current;
    if (shouldShowBlockingLoader) {
      setIsLoading(true);
    }
    if (!silent) {
      setError(null);
    }

    try {
      // Phase 1: Fetch diff first (fast, local) - show code diff immediately
      let files: DiffFile[] = [];
      let summary: DiffSummary = DEFAULT_SUMMARY;
      try {
        const diffData = await fetchDiffOnly(attemptId);
        files = diffData.files;
        summary = diffData.summary;
      } catch (err) {
        if (err instanceof ApiError && err.status === 404) {
          // No diff yet - valid state
        } else {
          throw err;
        }
      }

      // Show diff immediately with fallback branch info (don't wait for branch-status)
      const initialSnapshot = buildDiffCacheEntry(
        files,
        summary,
        null,
        isCompleteRef.current,
        attemptId,
        Date.now()
      );
      DIFF_CACHE.set(attemptId, initialSnapshot);
      const nextState = toState(initialSnapshot);
      setState(nextState);
      isCompleteRef.current = nextState.isComplete;
      hasLoadedRef.current = true;
      setError(null);

      if (shouldShowBlockingLoader) {
        setIsLoading(false);
      }

      // Phase 2: Fetch branch-status in background (can be slow - calls GitLab)
      // Update UI when it arrives without blocking initial render
      fetchBranchStatusOnly(attemptId)
        .then((branchStatus) => {
          const snapshot = buildDiffCacheEntry(
            files,
            summary,
            { status: 'fulfilled', value: branchStatus },
            isCompleteRef.current,
            attemptId,
            Date.now()
          );
          DIFF_CACHE.set(attemptId, snapshot);
          setState(toState(snapshot));
          isCompleteRef.current = snapshot.isComplete;
        })
        .catch(() => {
          // Branch-status failed (e.g. GitLab timeout) - keep diff visible with fallback
        });
    } catch (err) {
      if (!silent) {
        setError(err instanceof Error ? err.message : 'Failed to fetch diff');
      }
    } finally {
      if (shouldShowBlockingLoader) {
        setIsLoading(false);
      }
    }
  }, [attemptId]);

  useEffect(() => {
    if (!enabled || !attemptId) return;
    void fetchDiff(false);
  }, [enabled, attemptId, fetchDiff]);

  // Lightweight polling refresh for timeline-like live updates.
  useEffect(() => {
    if (!enabled || !attemptId || !realtime || state.isComplete) return;
    const timer = window.setInterval(() => {
      if (typeof document !== 'undefined' && document.visibilityState === 'hidden') {
        return;
      }
      void fetchDiff(true);
    }, POLL_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [enabled, attemptId, realtime, fetchDiff, state.isComplete]);

  return {
    files: Array.from(state.files.values()),
    summary: state.summary,
    branchInfo: state.branchInfo,
    availableActions: state.availableActions,
    isComplete: state.isComplete,
    isLoading,
    error,
    refresh: () => fetchDiff(false),
  };
}

export type { UseDiffReturn, UseDiffOptions };
