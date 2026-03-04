/**
 * GitActions - Merge, Create PR, and Rebase buttons
 *
 * Features:
 * - Merge button (direct push to target)
 * - Create PR button (GitLab MR creation)
 * - Rebase button (rebase onto target)
 * - Loading and disabled states
 * - Tooltips explaining actions
 */

import { memo, useState } from 'react';
import { clsx } from 'clsx';
import type { AvailableActions, BranchInfo } from './types';
import { apiPost, API_PREFIX } from '../../api/client';

interface GitActionsProps {
  attemptId?: string;
  availableActions: AvailableActions | null;
  branchInfo: BranchInfo | null;
  onMerge?: () => void;
  onCreatePR?: () => void;
  onRebase?: () => void;
  isLoading?: boolean;
}

export const GitActions = memo(function GitActions({
  attemptId,
  availableActions,
  branchInfo,
  onMerge,
  onCreatePR,
  onRebase,
  isLoading: externalLoading,
}: GitActionsProps) {
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const isLoading = externalLoading || actionLoading !== null;
  const actions = availableActions || { canMerge: false, canCreatePR: false, canRebase: false, canReject: true };

  const handleMerge = async () => {
    if (!attemptId) return;
    setActionLoading('merge');
    setError(null);

    try {
      await apiPost(`${API_PREFIX}/attempts/${attemptId}/approve`, {});
      onMerge?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to merge');
    } finally {
      setActionLoading(null);
    }
  };

  const handleCreatePR = async () => {
    if (!attemptId) return;
    setActionLoading('pr');
    setError(null);

    try {
      await apiPost(`${API_PREFIX}/attempts/${attemptId}/approve`, { create_pr: true });
      onCreatePR?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create PR');
    } finally {
      setActionLoading(null);
    }
  };

  const handleRebase = async () => {
    if (!attemptId) return;
    setActionLoading('rebase');
    setError(null);

    try {
      await apiPost(`${API_PREFIX}/attempts/${attemptId}/rebase`, {});
      onRebase?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to rebase');
    } finally {
      setActionLoading(null);
    }
  };

  const buttonBase =
    'inline-flex items-center gap-1.5 h-8 px-2.5 rounded-sm border text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed';

  return (
    <div className="space-y-2">
      {/* Error message */}
      {error && (
        <div className="px-2.5 py-2 bg-destructive/10 border border-destructive/30 rounded-sm text-xs text-destructive">
          {error}
        </div>
      )}

      {/* Action buttons */}
      <div className="flex flex-wrap items-center gap-2">
        {/* Merge button */}
        <button
          onClick={handleMerge}
          disabled={!actions.canMerge || isLoading || branchInfo?.hasConflicts}
          className={clsx(
            buttonBase,
            actions.canMerge && !isLoading && !branchInfo?.hasConflicts
              ? 'border-emerald-600/50 text-emerald-500 hover:bg-emerald-500/10'
              : 'border-border text-muted-foreground'
          )}
          title={
            branchInfo?.hasConflicts
              ? 'Cannot merge: conflicts detected'
              : !actions.canMerge
              ? 'Direct merge not available'
              : 'Merge changes to target branch'
          }
        >
          {actionLoading === 'merge' ? (
            <span className="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
          ) : (
            <span className="material-symbols-outlined text-[16px]">merge</span>
          )}
          Merge
        </button>

        {/* Create PR button */}
        <button
          onClick={handleCreatePR}
          disabled={!actions.canCreatePR || isLoading}
          className={clsx(
            buttonBase,
            actions.canCreatePR && !isLoading
              ? 'border-border text-foreground hover:bg-muted/50'
              : 'border-border text-muted-foreground'
          )}
          title={!actions.canCreatePR ? 'PR creation not available' : 'Create pull request on GitLab'}
        >
          {actionLoading === 'pr' ? (
            <span className="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
          ) : (
            <span className="material-symbols-outlined text-[16px]">rate_review</span>
          )}
          Create PR
        </button>

        {/* Rebase button */}
        <button
          onClick={handleRebase}
          disabled={!actions.canRebase || isLoading}
          className={clsx(
            buttonBase,
            actions.canRebase && !isLoading
              ? 'border-border text-muted-foreground hover:text-foreground hover:bg-muted/50'
              : 'border-border text-muted-foreground'
          )}
          title={!actions.canRebase ? 'Rebase not available' : 'Rebase onto target branch'}
        >
          {actionLoading === 'rebase' ? (
            <span className="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
          ) : (
            <span className="material-symbols-outlined text-[16px]">rebase</span>
          )}
          Rebase
        </button>
      </div>

      {/* Conflict warning */}
      {branchInfo?.hasConflicts && (
        <div className="flex items-center gap-2 px-2.5 py-2 bg-amber-500/10 border border-amber-500/30 rounded-sm text-xs text-amber-500">
          <span className="material-symbols-outlined text-[16px]">warning</span>
          <span>Conflicts detected. Please rebase or resolve conflicts before merging.</span>
        </div>
      )}

      {/* Behind warning */}
      {branchInfo && branchInfo.commitsBehind > 0 && !branchInfo.hasConflicts && (
        <div className="flex items-center gap-2 px-2.5 py-2 bg-muted/40 border border-border rounded-sm text-xs text-muted-foreground">
          <span className="material-symbols-outlined text-[16px]">info</span>
          <span>
            Branch is {branchInfo.commitsBehind} commit{branchInfo.commitsBehind > 1 ? 's' : ''} behind.
            Consider rebasing to get latest changes.
          </span>
        </div>
      )}
    </div>
  );
});
