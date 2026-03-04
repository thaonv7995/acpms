import { useGitOperationsError } from '../../contexts/GitOperationsContext';

export function GitErrorBanner() {
  const { error: gitError } = useGitOperationsError();

  if (!gitError) return null;

  return (
    <div className="mx-4 mt-4 p-3 border border-destructive rounded">
      <div className="text-destructive text-sm">{gitError}</div>
    </div>
  );
}
