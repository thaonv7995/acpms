import { useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  useGetAttempt,
  useGetTaskAttempts,
} from '../../api/generated/task-attempts/task-attempts';
import type { TaskAttempt } from '../../types/task-attempt';

/**
 * Custom hook for managing attempt data and redirects
 */
export function useAttemptData(
  taskId: string | undefined,
  attemptId: string | undefined,
  projectId: string | undefined
) {
  const navigate = useNavigate();

  // Fetch attempts list for latest redirect and switcher
  const { data: attemptsListResponse, isLoading: isAttemptsLoading } =
    useGetTaskAttempts(taskId!, {
      query: {
        enabled: !!taskId,
      },
    });

  // Get sorted attempts for switcher - map to TaskAttempt type
  const sortedAttempts = useMemo(() => {
    if (!attemptsListResponse?.data) return [];
    return [...attemptsListResponse.data]
      .map((attempt) => ({
        id: attempt.id,
        task_id: attempt.task_id,
        metadata: attempt.metadata as Record<string, unknown>,
        branch: (attempt.metadata as { branch?: string })?.branch,
        status: attempt.status.toLowerCase() as TaskAttempt['status'],
        started_at: attempt.started_at ?? undefined,
        completed_at: attempt.completed_at ?? undefined,
        ended_at: attempt.completed_at ?? undefined,
        error_message: attempt.error_message ?? undefined,
        created_at: attempt.created_at,
        updated_at: attempt.created_at, // API doesn't have updated_at
      }))
      .sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
  }, [attemptsListResponse]);

  // Get latest attempt ID
  const latestAttemptId = useMemo(() => {
    return sortedAttempts[0]?.id;
  }, [sortedAttempts]);

  // Handle /attempts/latest redirect
  useEffect(() => {
    if (attemptId === 'latest' && latestAttemptId) {
      // Use new URL structure: /tasks/projects/:projectId/:taskId/attempts/:attemptId
      if (projectId) {
        navigate(
          `/tasks/projects/${projectId}/${taskId}/attempts/${latestAttemptId}`,
          {
            replace: true,
          }
        );
      } else {
        // If no projectId, we're on /tasks (all projects)
        // Need to get projectId from task - for now, navigate without project context
        navigate(`/tasks/${taskId}/attempts/${latestAttemptId}`, {
          replace: true,
        });
      }
    }
  }, [attemptId, latestAttemptId, navigate, projectId, taskId]);

  // Only redirect when attemptId is explicitly 'latest', not when it's undefined
  // When attemptId is undefined, we want to show TaskPanel with attempts list
  // This matches vibe-kanban-reference behavior

  // Fetch attempt data from API when attemptId exists (not 'latest')
  const { data: attemptResponse } = useGetAttempt(attemptId!, {
    query: {
      enabled: !!attemptId && attemptId !== 'latest',
    },
  });

  // Map API response to TaskAttempt type
  const selectedAttempt: TaskAttempt | null = useMemo(() => {
    if (!attemptId || !attemptResponse?.data) return null;
    const attempt = attemptResponse.data;
    return {
      id: attempt.id,
      task_id: attempt.task_id,
      metadata: attempt.metadata as Record<string, unknown>,
      branch: (attempt.metadata as { branch?: string })?.branch,
      status: attempt.status.toLowerCase() as TaskAttempt['status'],
      started_at: attempt.started_at ?? undefined,
      completed_at: attempt.completed_at ?? undefined,
      ended_at: attempt.completed_at ?? undefined,
      error_message: attempt.error_message ?? undefined,
      created_at: attempt.created_at,
      updated_at: attempt.created_at, // API doesn't have updated_at
    };
  }, [attemptId, attemptResponse]);

  return {
    sortedAttempts,
    selectedAttempt,
    isAttemptsLoading,
  };
}
