/**
 * useNavigateWithSearch - Navigation hook that preserves URL search params
 *
 * When navigating between views (task → attempt → task), this hook ensures
 * that query parameters like ?view=diffs are preserved.
 */
import { useCallback } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';

export function useNavigateWithSearch() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  return useCallback(
    (path: string, options?: { replace?: boolean }) => {
      const search = searchParams.toString();
      const fullPath = search ? `${path}?${search}` : path;
      navigate(fullPath, options);
    },
    [navigate, searchParams]
  );
}
