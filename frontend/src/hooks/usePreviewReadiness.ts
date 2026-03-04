import { useQuery } from '@tanstack/react-query';
import { getPreviewReadiness, type PreviewReadiness } from '@/api/previews';

interface UsePreviewReadinessResult {
  readiness: PreviewReadiness | null;
  isLoading: boolean;
  errorMessage?: string;
}

export function usePreviewReadiness(attemptId?: string): UsePreviewReadinessResult {
  const { data, isLoading, error } = useQuery({
    queryKey: ['preview-readiness', attemptId],
    queryFn: () => getPreviewReadiness(attemptId!),
    enabled: Boolean(attemptId),
    staleTime: 30_000,
  });

  return {
    readiness: data ?? null,
    isLoading,
    errorMessage: error instanceof Error ? error.message : undefined,
  };
}

