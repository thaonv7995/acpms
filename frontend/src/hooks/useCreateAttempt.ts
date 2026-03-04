import { useMutation, useQueryClient } from '@tanstack/react-query';
import { apiPost, API_PREFIX } from '@/api/client';
import type { TaskAttempt, CreateAttemptRequest } from '@/types/task-attempt';

interface UseCreateAttemptOptions {
  onSuccess?: (attempt: TaskAttempt) => void;
  onError?: (error: Error) => void;
}

export function useCreateAttempt(options: UseCreateAttemptOptions = {}) {
  const queryClient = useQueryClient();
  const { onSuccess, onError } = options;

  return useMutation({
    mutationFn: async (data: CreateAttemptRequest) => {
      return apiPost<TaskAttempt>(
        `${API_PREFIX}/tasks/${data.task_id}/attempts`,
        {
          executor: data.executor,
          variant: data.variant,
          base_branch: data.base_branch,
          prompt: data.prompt,
        }
      );
    },
    onSuccess: (attempt) => {
      // Invalidate attempts list so it refetches
      queryClient.invalidateQueries({
        queryKey: ['tasks', attempt.task_id, 'attempts'],
      });
      onSuccess?.(attempt);
    },
    onError: (error: Error) => {
      onError?.(error);
    },
  });
}
