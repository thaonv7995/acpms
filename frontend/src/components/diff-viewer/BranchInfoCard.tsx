/**
 * BranchInfoCard - Shows branch comparison info and git actions
 *
 * Features:
 * - Source and target branch display
 * - Commits ahead/behind status
 * - Merge, Create PR, Rebase action buttons
 */

import { memo } from 'react';
import type { BranchInfo, AvailableActions } from './types';
import { GitActions } from './GitActions';

interface BranchInfoCardProps {
  branchInfo: BranchInfo | null;
  availableActions: AvailableActions | null;
  attemptId?: string;
  onMerge?: () => void;
  onCreatePR?: () => void;
  onRebase?: () => void;
  isLoading?: boolean;
}

export const BranchInfoCard = memo(function BranchInfoCard({
  branchInfo,
  availableActions,
  attemptId,
  onMerge,
  onCreatePR,
  onRebase,
  isLoading,
}: BranchInfoCardProps) {
  if (!branchInfo) {
    return (
      <div className="border border-border bg-background px-3 py-2">
        <div className="flex items-center gap-2 text-muted-foreground">
          <span className="material-symbols-outlined text-[18px] animate-spin">progress_activity</span>
          <span className="text-sm">Loading branch info...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="border border-border bg-background overflow-hidden">
      {/* Branch comparison */}
      <div className="px-3 py-2 border-b border-border">
        <div className="flex flex-wrap items-center gap-2">
          {/* Source branch */}
          <span className="inline-flex items-center gap-1.5 px-2 py-1 rounded-sm border border-border bg-muted/30 font-mono text-xs text-foreground">
            <span className="material-symbols-outlined text-[14px]">fork_right</span>
            {branchInfo.source}
          </span>

          {/* Arrow */}
          <span className="material-symbols-outlined text-[16px] text-muted-foreground">
            arrow_forward
          </span>

          {/* Target branch */}
          <span className="inline-flex items-center gap-1.5 px-2 py-1 rounded-sm border border-border bg-muted/30 font-mono text-xs text-foreground">
            <span className="material-symbols-outlined text-[14px]">fork_right</span>
            {branchInfo.target}
          </span>

          {/* Commit status */}
          <div className="flex items-center gap-2 ml-auto">
            {branchInfo.commitsAhead > 0 && (
              <span className="inline-flex items-center gap-1 text-xs text-emerald-500">
                <span className="material-symbols-outlined text-[14px]">arrow_upward</span>
                {branchInfo.commitsAhead} ahead
              </span>
            )}
            {branchInfo.commitsBehind > 0 && (
              <span className="inline-flex items-center gap-1 text-xs text-amber-500">
                <span className="material-symbols-outlined text-[14px]">arrow_downward</span>
                {branchInfo.commitsBehind} behind
              </span>
            )}
            {branchInfo.hasConflicts && (
              <span className="inline-flex items-center gap-1 text-xs text-red-500">
                <span className="material-symbols-outlined text-[14px]">warning</span>
                Conflicts
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Git Actions */}
      <div className="px-3 py-2">
        <GitActions
          attemptId={attemptId}
          availableActions={availableActions}
          branchInfo={branchInfo}
          onMerge={onMerge}
          onCreatePR={onCreatePR}
          onRebase={onRebase}
          isLoading={isLoading}
        />
      </div>
    </div>
  );
});
