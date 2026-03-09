// Hook for fetching diffs via REST API with React Query
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getApiBaseUrl } from '../api/client';
import type { FileDiff, DiffSummary, DiffResponse, BranchStatus } from '../types/diff';

const API_BASE = getApiBaseUrl();

// API functions
async function fetchDiffs(attemptId: string): Promise<DiffResponse> {
  const response = await fetch(`${API_BASE}/api/v1/attempts/${attemptId}/diffs`, {
    headers: {
      Authorization: `Bearer ${localStorage.getItem('acpms_token')}`,
    },
  });
  if (!response.ok) throw new Error('Failed to fetch diffs');
  return response.json();
}

async function fetchBranchStatus(attemptId: string): Promise<BranchStatus> {
  const response = await fetch(`${API_BASE}/api/v1/attempts/${attemptId}/branch-status`, {
    headers: {
      Authorization: `Bearer ${localStorage.getItem('acpms_token')}`,
    },
  });
  if (!response.ok) throw new Error('Failed to fetch branch status');
  return response.json();
}

async function pushBranch(attemptId: string): Promise<void> {
  const response = await fetch(`${API_BASE}/api/v1/attempts/${attemptId}/push`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${localStorage.getItem('acpms_token')}`,
    },
  });
  if (!response.ok) throw new Error('Failed to push branch');
}

async function createPullRequest(attemptId: string): Promise<{ pr_url: string }> {
  const response = await fetch(`${API_BASE}/api/v1/attempts/${attemptId}/pull-request`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${localStorage.getItem('acpms_token')}`,
    },
  });
  if (!response.ok) throw new Error('Failed to create pull request');
  return response.json();
}

// Query keys
export const diffKeys = {
  all: ['diffs'] as const,
  attempt: (attemptId: string) => [...diffKeys.all, attemptId] as const,
  branchStatus: (attemptId: string) => [...diffKeys.all, attemptId, 'branch-status'] as const,
};

// Main hook for diffs
export function useDiffs(attemptId: string | undefined) {
  const queryClient = useQueryClient();

  const diffsQuery = useQuery({
    queryKey: diffKeys.attempt(attemptId!),
    queryFn: () => fetchDiffs(attemptId!),
    enabled: !!attemptId,
    staleTime: 30000, // 30 seconds
  });

  const branchStatusQuery = useQuery({
    queryKey: diffKeys.branchStatus(attemptId!),
    queryFn: () => fetchBranchStatus(attemptId!),
    enabled: !!attemptId,
    staleTime: 10000, // 10 seconds
  });

  const pushMutation = useMutation({
    mutationFn: () => pushBranch(attemptId!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: diffKeys.branchStatus(attemptId!) });
    },
  });

  const createPrMutation = useMutation({
    mutationFn: () => createPullRequest(attemptId!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: diffKeys.branchStatus(attemptId!) });
    },
  });

  return {
    diffs: diffsQuery.data?.diffs ?? [],
    summary: diffsQuery.data?.summary,
    branchStatus: branchStatusQuery.data,
    isLoading: diffsQuery.isLoading,
    isError: diffsQuery.isError,
    error: diffsQuery.error,
    refetch: diffsQuery.refetch,
    push: pushMutation.mutate,
    isPushing: pushMutation.isPending,
    createPr: createPrMutation.mutate,
    isCreatingPr: createPrMutation.isPending,
    prUrl: createPrMutation.data?.pr_url,
  };
}

// Hook for calculating diff summary from diffs array
export function useDiffSummary(diffs: FileDiff[]): DiffSummary {
  return useMemo(() => {
    const summary: DiffSummary = {
      total_files: diffs.length,
      total_additions: 0,
      total_deletions: 0,
      files_added: 0,
      files_modified: 0,
      files_deleted: 0,
      files_renamed: 0,
    };

    for (const diff of diffs) {
      summary.total_additions += diff.additions;
      summary.total_deletions += diff.deletions;

      switch (diff.status) {
        case 'added':
          summary.files_added++;
          break;
        case 'modified':
          summary.files_modified++;
          break;
        case 'deleted':
          summary.files_deleted++;
          break;
        case 'renamed':
          summary.files_renamed++;
          break;
      }
    }

    return summary;
  }, [diffs]);
}
