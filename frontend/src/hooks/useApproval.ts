import { useState, useCallback } from 'react';
import { respondToApproval } from '@/api/approvals';

export interface UseApprovalReturn {
  approve: () => Promise<void>;
  deny: (reason?: string) => Promise<void>;
  isApproving: boolean;
  isDenying: boolean;
  error: string | null;
}

export function useApproval(approvalId: string): UseApprovalReturn {
  const [isApproving, setIsApproving] = useState(false);
  const [isDenying, setIsDenying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const approve = useCallback(async () => {
    if (isApproving || isDenying) return;

    setIsApproving(true);
    setError(null);

    try {
      await respondToApproval(approvalId, 'approve');
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : 'Failed to approve';
      setError(errorMessage);
      throw err;
    } finally {
      setIsApproving(false);
    }
  }, [approvalId, isApproving, isDenying]);

  const deny = useCallback(async (reason?: string) => {
    if (isApproving || isDenying) return;

    setIsDenying(true);
    setError(null);

    try {
      await respondToApproval(approvalId, 'deny', reason);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : 'Failed to deny';
      setError(errorMessage);
      throw err;
    } finally {
      setIsDenying(false);
    }
  }, [approvalId, isApproving, isDenying]);

  return {
    approve,
    deny,
    isApproving,
    isDenying,
    error,
  };
}
