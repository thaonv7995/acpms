import { useQuery } from '@tanstack/react-query';
import { apiGet, API_PREFIX } from '@/api/client';
import type { TaskAttempt } from '@/types/task-attempt';

interface UseTaskAttemptsOptions {
  enabled?: boolean;
  refetchInterval?: number;
}

export function useTaskAttempts(
  taskId: string,
  options: UseTaskAttemptsOptions = {}
) {
  const { enabled = true, refetchInterval } = options;

  return useQuery({
    queryKey: ['tasks', taskId, 'attempts'],
    queryFn: async () => {
      return apiGet<TaskAttempt[]>(`${API_PREFIX}/tasks/${taskId}/attempts`);
    },
    enabled: enabled && !!taskId,
    refetchInterval,
    staleTime: 5 * 60 * 1000, // 5 minutes
  });
}

// Helper hook to get the latest attempt
export function useLatestAttempt(taskId: string) {
  const { data: attempts, ...rest } = useTaskAttempts(taskId);

  const latestAttempt = attempts?.length
    ? [...attempts].sort(
        (a, b) =>
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      )[0]
    : undefined;

  return {
    ...rest,
    data: latestAttempt,
    attempts,
  };
}
