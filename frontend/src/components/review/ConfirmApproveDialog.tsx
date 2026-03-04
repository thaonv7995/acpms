// ConfirmApproveDialog - Confirmation dialog for approving changes
import { useState } from 'react';

interface ConfirmApproveDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (commitMessage?: string) => Promise<void>;
  /** When approve fails with merge conflict, call this to open Request Changes with preset */
  onResolveConflicts?: () => void;
  isLoading?: boolean;
  taskTitle?: string;
  hasUnresolvedComments?: boolean;
  unresolvedCount?: number;
}

/**
 * Dialog for confirming approval of code changes.
 * Allows entering optional commit message before approval.
 *
 * Usage:
 * <ConfirmApproveDialog
 *   isOpen={showApproveDialog}
 *   onClose={() => setShowApproveDialog(false)}
 *   onConfirm={handleApprove}
 * />
 */
export function ConfirmApproveDialog({
  isOpen,
  onClose,
  onConfirm,
  onResolveConflicts,
  isLoading = false,
  taskTitle,
  hasUnresolvedComments = false,
  unresolvedCount = 0,
}: ConfirmApproveDialogProps) {
  const [commitMessage, setCommitMessage] = useState('');
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleConfirm = async () => {
    setError(null);
    try {
      await onConfirm(commitMessage || undefined);
      setCommitMessage('');
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to approve');
    }
  };

  const handleClose = () => {
    if (!isLoading) {
      setCommitMessage('');
      setError(null);
      onClose();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-[2px]"
        onClick={handleClose}
      />
      <div className="relative w-full max-w-md bg-card border border-border rounded-xl shadow-2xl overflow-hidden">
        <div className="p-6">
          {/* Header */}
          <div className="flex items-center gap-3 mb-4">
            <div className="size-10 rounded-lg bg-[hsl(var(--success))]/20 flex items-center justify-center">
              <span className="material-symbols-outlined text-[hsl(var(--success))]">check_circle</span>
            </div>
            <div>
              <h2 className="text-lg font-bold text-card-foreground">
                Approve Changes
              </h2>
              {taskTitle && (
                <p className="text-xs text-muted-foreground truncate mt-0.5">
                  {taskTitle}
                </p>
              )}
            </div>
          </div>

          {/* Warning for unresolved comments */}
          {hasUnresolvedComments && (
            <div className="mb-4 p-3 bg-amber-50 dark:bg-amber-500/20 border border-amber-200 dark:border-amber-500/30 rounded-lg">
              <div className="flex items-start gap-2">
                <span className="material-symbols-outlined text-amber-600 dark:text-amber-400 text-[18px] mt-0.5 shrink-0">
                  warning
                </span>
                <div>
                  <p className="text-sm font-medium text-amber-800 dark:text-amber-300">
                    Unresolved comments
                  </p>
                  <p className="text-xs text-amber-700 dark:text-amber-400/80">
                    There {unresolvedCount === 1 ? 'is' : 'are'} {unresolvedCount} unresolved{' '}
                    {unresolvedCount === 1 ? 'comment' : 'comments'}. You can still approve,
                    but consider addressing them first.
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* Description */}
          <p className="text-sm text-muted-foreground mb-4">
            Approving will push the changes to the repository and mark the task as complete.
            This action cannot be undone.
          </p>

          {/* Commit Message Input */}
          <div className="mb-4">
            <label className="block text-sm font-medium text-card-foreground mb-1.5">
              Commit Message <span className="text-muted-foreground font-normal">(optional)</span>
            </label>
            <input
              type="text"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Approved changes for task..."
              disabled={isLoading}
              className="w-full px-3 py-2 text-sm bg-muted border border-border rounded-lg text-card-foreground placeholder-muted-foreground focus:ring-1 focus:ring-primary focus:border-primary disabled:opacity-50"
            />
          </div>

          {/* Error */}
          {error && (
            <div className="mb-4 p-3 bg-destructive/10 border border-destructive/30 rounded-lg flex items-start gap-2">
              <span className="material-symbols-outlined text-destructive text-[18px] shrink-0">error</span>
              <div className="flex-1 min-w-0">
                <p className="text-sm text-destructive font-medium">{error}</p>
                {error.toLowerCase().includes('internal server error') && (
                  <p className="text-xs text-muted-foreground mt-1">
                    Check server logs or contact admin. May be due to GitLab not configured, worktree missing, or connection error.
                  </p>
                )}
                {onResolveConflicts && (error.toLowerCase().includes('merge failed') || error.toLowerCase().includes('conflict')) && (
                  <button
                    type="button"
                    onClick={() => {
                      onResolveConflicts();
                      setError(null);
                      onClose();
                    }}
                    className="mt-2 px-3 py-1.5 text-xs font-medium rounded-lg bg-warning/20 text-warning border border-warning/40 hover:bg-warning/30 transition-colors"
                  >
                    Request agent to resolve conflicts
                  </button>
                )}
              </div>
            </div>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-3">
            <button
              onClick={handleClose}
              disabled={isLoading}
              className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={handleConfirm}
              disabled={isLoading}
              className="px-5 py-2 text-sm font-medium rounded-lg bg-[hsl(var(--success))] text-white hover:opacity-90 transition-opacity disabled:opacity-50 flex items-center gap-2"
            >
              {isLoading ? (
                <>
                  <div className="animate-spin rounded-full h-4 w-4 border-2 border-current border-t-transparent" />
                  Approving...
                </>
              ) : (
                <>
                  <span className="material-symbols-outlined text-[18px]">check</span>
                  Approve & Push
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
