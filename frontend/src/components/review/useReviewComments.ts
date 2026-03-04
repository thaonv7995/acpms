// Hook for review comments CRUD operations
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import { apiGet, apiPost, apiPostFull, apiPatch, API_PREFIX } from '../../api/client';
import type {
  ReviewComment,
  AddCommentRequest,
  RejectRequest,
  RequestChangesRequest,
  ApproveRequest,
  CommentsByFile,
  ReviewStatus,
} from './types';

// API functions
async function fetchComments(attemptId: string): Promise<ReviewComment[]> {
  return apiGet<ReviewComment[]>(`${API_PREFIX}/attempts/${attemptId}/comments`);
}

async function addComment(request: AddCommentRequest): Promise<ReviewComment> {
  return apiPost<ReviewComment>(
    `${API_PREFIX}/attempts/${request.attempt_id}/comments`,
    {
      content: request.content,
      file_path: request.file_path,
      line_number: request.line_number,
    }
  );
}

async function resolveComment(commentId: string): Promise<void> {
  return apiPatch<void>(`${API_PREFIX}/comments/${commentId}/resolve`, {});
}

async function unresolveComment(commentId: string): Promise<void> {
  return apiPatch<void>(`${API_PREFIX}/comments/${commentId}/unresolve`, {});
}

async function approveAttempt(attemptId: string, request: ApproveRequest): Promise<string> {
  const res = await apiPostFull<unknown>(`${API_PREFIX}/attempts/${attemptId}/approve`, request);
  return res.message ?? 'Changes approved successfully';
}

async function rejectAttempt(attemptId: string, request: RejectRequest): Promise<void> {
  return apiPost<void>(`${API_PREFIX}/attempts/${attemptId}/reject`, request);
}

async function requestChanges(attemptId: string, request: RequestChangesRequest): Promise<void> {
  return apiPost<void>(`${API_PREFIX}/attempts/${attemptId}/request-changes`, request);
}

// Query keys
export const reviewKeys = {
  all: ['review-comments'] as const,
  comments: (attemptId: string) => [...reviewKeys.all, attemptId] as const,
};

/**
 * Hook for managing review comments
 * Provides CRUD operations and derived state for comments
 */
export function useReviewComments(attemptId: string | undefined) {
  const queryClient = useQueryClient();

  // Fetch comments query
  const commentsQuery = useQuery({
    queryKey: reviewKeys.comments(attemptId!),
    queryFn: () => fetchComments(attemptId!),
    enabled: !!attemptId,
    staleTime: 10000, // 10 seconds
  });

  // Add comment mutation
  const addCommentMutation = useMutation({
    mutationFn: (request: Omit<AddCommentRequest, 'attempt_id'>) =>
      addComment({ ...request, attempt_id: attemptId! }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: reviewKeys.comments(attemptId!) });
    },
  });

  // Resolve comment mutation
  const resolveCommentMutation = useMutation({
    mutationFn: resolveComment,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: reviewKeys.comments(attemptId!) });
    },
  });

  // Unresolve comment mutation
  const unresolveCommentMutation = useMutation({
    mutationFn: unresolveComment,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: reviewKeys.comments(attemptId!) });
    },
  });

  // Approve mutation
  const approveMutation = useMutation({
    mutationFn: (request: ApproveRequest) => approveAttempt(attemptId!, request),
  });

  // Reject mutation
  const rejectMutation = useMutation({
    mutationFn: (request: RejectRequest) => rejectAttempt(attemptId!, request),
  });

  // Request changes mutation
  const requestChangesMutation = useMutation({
    mutationFn: (request: RequestChangesRequest) => requestChanges(attemptId!, request),
  });

  // Derived state: comments grouped by file
  const commentsByFile = useMemo<CommentsByFile[]>(() => {
    const comments = commentsQuery.data ?? [];
    const grouped = new Map<string | null, ReviewComment[]>();

    // Group comments by file path
    comments.forEach((comment) => {
      const key = comment.file_path ?? null;
      const existing = grouped.get(key) ?? [];
      grouped.set(key, [...existing, comment]);
    });

    // Convert to array and sort (general comments first, then by file path)
    const result: CommentsByFile[] = [];

    // Add general comments first
    const generalComments = grouped.get(null);
    if (generalComments && generalComments.length > 0) {
      result.push({ filePath: null, comments: generalComments });
    }

    // Add file-specific comments
    grouped.forEach((comments, filePath) => {
      if (filePath !== null) {
        result.push({
          filePath,
          comments: comments.sort((a, b) => {
            // Sort by line number within file
            const lineA = a.line_number ?? 0;
            const lineB = b.line_number ?? 0;
            return lineA - lineB;
          }),
        });
      }
    });

    return result;
  }, [commentsQuery.data]);

  // Derived state: comments for a specific line
  const getLineComments = (filePath: string, lineNumber: number): ReviewComment[] => {
    return (commentsQuery.data ?? []).filter(
      (c) => c.file_path === filePath && c.line_number === lineNumber
    );
  };

  // Derived state: review status
  const reviewStatus = useMemo<ReviewStatus>(() => {
    const comments = commentsQuery.data ?? [];
    const totalComments = comments.length;
    const resolvedComments = comments.filter((c) => c.resolved).length;
    const hasUnresolvedComments = resolvedComments < totalComments;

    return {
      hasUnresolvedComments,
      totalComments,
      resolvedComments,
      canApprove: true, // Can approve even with unresolved comments (policy decision)
    };
  }, [commentsQuery.data]);

  return {
    // Data
    comments: commentsQuery.data ?? [],
    commentsByFile,
    reviewStatus,
    getLineComments,

    // Loading states
    isLoading: commentsQuery.isLoading,
    isError: commentsQuery.isError,
    error: commentsQuery.error,

    // Comment operations
    addComment: addCommentMutation.mutateAsync,
    isAddingComment: addCommentMutation.isPending,
    addCommentError: addCommentMutation.error,

    resolveComment: resolveCommentMutation.mutateAsync,
    isResolvingComment: resolveCommentMutation.isPending,

    unresolveComment: unresolveCommentMutation.mutateAsync,
    isUnresolvingComment: unresolveCommentMutation.isPending,

    // Review actions
    approve: approveMutation.mutateAsync,
    isApproving: approveMutation.isPending,
    approveError: approveMutation.error,

    reject: rejectMutation.mutateAsync,
    isRejecting: rejectMutation.isPending,
    rejectError: rejectMutation.error,

    requestChanges: requestChangesMutation.mutateAsync,
    isRequestingChanges: requestChangesMutation.isPending,
    requestChangesError: requestChangesMutation.error,

    // Utilities
    refetch: commentsQuery.refetch,
  };
}

/**
 * Hook for review actions only (approve/reject/request changes)
 * Use when you only need action buttons without comments
 */
export function useReviewActions(attemptId: string | undefined) {
  const approveMutation = useMutation({
    mutationFn: (request: ApproveRequest) => approveAttempt(attemptId!, request),
  });

  const rejectMutation = useMutation({
    mutationFn: (request: RejectRequest) => rejectAttempt(attemptId!, request),
  });

  const requestChangesMutation = useMutation({
    mutationFn: (request: RequestChangesRequest) => requestChanges(attemptId!, request),
  });

  const isLoading =
    approveMutation.isPending ||
    rejectMutation.isPending ||
    requestChangesMutation.isPending;

  return {
    approve: approveMutation.mutateAsync,
    isApproving: approveMutation.isPending,
    approveError: approveMutation.error,

    reject: rejectMutation.mutateAsync,
    isRejecting: rejectMutation.isPending,
    rejectError: rejectMutation.error,

    requestChanges: requestChangesMutation.mutateAsync,
    isRequestingChanges: requestChangesMutation.isPending,
    requestChangesError: requestChangesMutation.error,

    isLoading,
  };
}
