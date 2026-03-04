import { useCallback, useEffect, useState } from 'react';
import { getProjectMembers, type ProjectMember } from '../api/projects';
import { logger } from '@/lib/logger';

export function useProjectMembers(projectId?: string) {
  const [members, setMembers] = useState<ProjectMember[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchMembers = useCallback(async () => {
    if (!projectId) {
      setMembers([]);
      setError(null);
      return;
    }

    setLoading(true);
    try {
      const data = await getProjectMembers(projectId);
      setMembers(data);
      setError(null);
    } catch (err) {
      logger.error('Failed to fetch project members:', err);
      setMembers([]);
      setError('Failed to fetch project members');
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    void fetchMembers();
  }, [fetchMembers]);

  return {
    members,
    setMembers,
    loading,
    error,
    refetch: fetchMembers,
  };
}
