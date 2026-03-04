// ReviewActions - Action buttons bar for code review (Approve/Reject/Request Changes)
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import type { RequestChangesRequest, ReviewStatus } from './types';
import { ConfirmApproveDialog } from './ConfirmApproveDialog';
import { ConfirmRejectDialog } from './ConfirmRejectDialog';
import { RequestChangesDialog } from './RequestChangesDialog';

interface ReviewActionsProps {
  attemptId: string;
  taskTitle?: string;
  reviewStatus?: ReviewStatus;
  onApprove: (commitMessage?: string) => Promise<void>;
  onReject: (reason: string) => Promise<void>;
  onRequestChanges: (request: RequestChangesRequest) => Promise<void>;
  isApproving?: boolean;
  isRejecting?: boolean;
  isRequestingChanges?: boolean;
  disabled?: boolean;
  compact?: boolean;
}

/**
 * Action buttons bar for code review workflow.
 * Displays Approve, Reject, and Request Changes buttons with confirmation dialogs.
 *
 * Usage:
 * <ReviewActions
 *   attemptId={attemptId}
 *   onApprove={approve}
 *   onReject={reject}
 *   onRequestChanges={requestChanges}
 * />
 */
export function ReviewActions({
  attemptId: _attemptId,
  taskTitle,
  reviewStatus,
  onApprove,
  onReject,
  onRequestChanges,
  isApproving = false,
  isRejecting = false,
  isRequestingChanges = false,
  disabled = false,
  compact = false,
}: ReviewActionsProps) {
  const [showApproveDialog, setShowApproveDialog] = useState(false);
  const [showRejectDialog, setShowRejectDialog] = useState(false);
  const [showRequestChangesDialog, setShowRequestChangesDialog] = useState(false);
  const [requestChangesInitialFeedback, setRequestChangesInitialFeedback] = useState('');

  const isLoading = isApproving || isRejecting || isRequestingChanges;
  const isDisabled = disabled || isLoading;

  const unresolvedCount = reviewStatus
    ? reviewStatus.totalComments - reviewStatus.resolvedComments
    : 0;

  return (
    <>
      <div
        className={`
          flex items-center justify-end gap-3
          ${compact ? 'p-3' : 'px-6 py-4'}
          bg-muted/30 border-t border-border
        `}
      >
        {/* Review Status Indicator */}
        {reviewStatus && reviewStatus.totalComments > 0 && (
          <div className="flex-1 flex items-center gap-2 text-sm text-muted-foreground">
            <span className="material-symbols-outlined text-[18px]">comment</span>
            <span>
              {reviewStatus.resolvedComments}/{reviewStatus.totalComments} comments resolved
            </span>
            {unresolvedCount > 0 && (
              <span className="px-1.5 py-0.5 text-[10px] font-medium text-warning bg-warning/15 border border-warning/30 rounded">
                {unresolvedCount} pending
              </span>
            )}
          </div>
        )}

        {/* Action Buttons - Muted palette to match dark system */}
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size={compact ? 'sm' : 'default'}
            onClick={() => setShowRequestChangesDialog(true)}
            disabled={isDisabled}
            className="gap-2 border-border bg-transparent text-muted-foreground hover:bg-muted hover:border-warning/40 hover:text-warning/90"
          >
            {isRequestingChanges ? (
              <span className="animate-spin rounded-full h-4 w-4 border-2 border-warning/70 border-t-transparent" />
            ) : (
              <span className="material-symbols-outlined text-[18px]">edit_note</span>
            )}
            <span className={compact ? 'hidden sm:inline' : ''}>Request Changes</span>
          </Button>

          <Button
            variant="outline"
            size={compact ? 'sm' : 'default'}
            onClick={() => setShowRejectDialog(true)}
            disabled={isDisabled}
            className="gap-2 border-border bg-transparent text-muted-foreground hover:bg-muted hover:border-destructive/40 hover:text-destructive/90"
          >
            {isRejecting ? (
              <span className="animate-spin rounded-full h-4 w-4 border-2 border-destructive/70 border-t-transparent" />
            ) : (
              <span className="material-symbols-outlined text-[18px]">close</span>
            )}
            <span className={compact ? 'hidden sm:inline' : ''}>Reject</span>
          </Button>

          <Button
            variant="default"
            size={compact ? 'sm' : 'default'}
            onClick={() => setShowApproveDialog(true)}
            disabled={isDisabled}
            className="gap-2"
          >
            {isApproving ? (
              <span className="animate-spin rounded-full h-4 w-4 border-2 border-primary-foreground border-t-transparent" />
            ) : (
              <span className="material-symbols-outlined text-[18px]">check_circle</span>
            )}
            Approve
          </Button>
        </div>
      </div>

      {/* Confirmation Dialogs */}
      <ConfirmApproveDialog
        isOpen={showApproveDialog}
        onClose={() => setShowApproveDialog(false)}
        onConfirm={async (commitMessage) => {
          await onApprove(commitMessage);
          setShowApproveDialog(false);
        }}
        onResolveConflicts={() => {
          setShowApproveDialog(false);
          setShowRequestChangesDialog(true);
          setRequestChangesInitialFeedback(
            'Merge failed due to conflicts. Please pull main, resolve conflicts, and push again.'
          );
        }}
        isLoading={isApproving}
        taskTitle={taskTitle}
        hasUnresolvedComments={unresolvedCount > 0}
        unresolvedCount={unresolvedCount}
      />

      <ConfirmRejectDialog
        isOpen={showRejectDialog}
        onClose={() => setShowRejectDialog(false)}
        onConfirm={async (reason) => {
          await onReject(reason);
          setShowRejectDialog(false);
        }}
        isLoading={isRejecting}
        taskTitle={taskTitle}
      />

      <RequestChangesDialog
        isOpen={showRequestChangesDialog}
        onClose={() => {
          setShowRequestChangesDialog(false);
          setRequestChangesInitialFeedback('');
        }}
        onConfirm={async (request) => {
          await onRequestChanges(request);
          setShowRequestChangesDialog(false);
          setRequestChangesInitialFeedback('');
        }}
        initialFeedback={requestChangesInitialFeedback}
        isLoading={isRequestingChanges}
        taskTitle={taskTitle}
        unresolvedCommentCount={unresolvedCount}
      />
    </>
  );
}
