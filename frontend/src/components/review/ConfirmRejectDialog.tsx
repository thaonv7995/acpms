// ConfirmRejectDialog - Confirmation dialog for rejecting changes with reason
import { useState } from 'react';

interface ConfirmRejectDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (reason: string) => Promise<void>;
  isLoading?: boolean;
  taskTitle?: string;
}

/**
 * Dialog for rejecting code changes with a required reason.
 * The task will be moved back to Todo status.
 *
 * Usage:
 * <ConfirmRejectDialog
 *   isOpen={showRejectDialog}
 *   onClose={() => setShowRejectDialog(false)}
 *   onConfirm={handleReject}
 * />
 */
export function ConfirmRejectDialog({
  isOpen,
  onClose,
  onConfirm,
  isLoading = false,
  taskTitle,
}: ConfirmRejectDialogProps) {
  const [reason, setReason] = useState('');
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleConfirm = async () => {
    const trimmedReason = reason.trim();
    if (!trimmedReason) {
      setError('Please provide a reason for rejection');
      return;
    }

    setError(null);
    try {
      await onConfirm(trimmedReason);
      setReason('');
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reject');
    }
  };

  const handleClose = () => {
    if (!isLoading) {
      setReason('');
      setError(null);
      onClose();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={handleClose}
      />
      <div className="relative w-full max-w-md bg-white dark:bg-[#0d1117] border border-slate-200 dark:border-slate-700 rounded-2xl shadow-2xl overflow-hidden">
        <div className="p-6">
          {/* Header */}
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 rounded-lg bg-red-100 dark:bg-red-900/30 text-red-600">
              <span className="material-symbols-outlined">cancel</span>
            </div>
            <div>
              <h2 className="text-lg font-bold text-slate-900 dark:text-white">
                Reject Changes
              </h2>
              {taskTitle && (
                <p className="text-xs text-slate-500 dark:text-slate-400 truncate">
                  {taskTitle}
                </p>
              )}
            </div>
          </div>

          {/* Description */}
          <p className="text-sm text-slate-600 dark:text-slate-400 mb-4">
            Rejecting will discard the changes and move the task back to Todo.
            The worktree will be cleaned up.
          </p>

          {/* Reason Input */}
          <div className="mb-4">
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5">
              Rejection Reason <span className="text-red-500">*</span>
            </label>
            <textarea
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="Explain why these changes are being rejected..."
              disabled={isLoading}
              rows={3}
              className={`
                w-full px-3 py-2 text-sm
                border rounded-lg
                bg-white dark:bg-slate-800
                text-slate-900 dark:text-slate-100
                placeholder-slate-400
                focus:ring-2 focus:border-transparent
                disabled:opacity-50
                resize-none
                ${error && !reason.trim()
                  ? 'border-red-500 focus:ring-red-500'
                  : 'border-slate-200 dark:border-slate-700 focus:ring-red-500'
                }
              `}
            />
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              This reason will be logged and shown to the task assignee.
            </p>
          </div>

          {/* Error */}
          {error && (
            <div className="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
              <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
            </div>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-3">
            <button
              onClick={handleClose}
              disabled={isLoading}
              className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={handleConfirm}
              disabled={isLoading || !reason.trim()}
              className="px-5 py-2 text-white text-sm font-bold rounded-lg bg-red-600 hover:bg-red-700 shadow-lg shadow-red-600/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
            >
              {isLoading ? (
                <>
                  <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                  Rejecting...
                </>
              ) : (
                <>
                  <span className="material-symbols-outlined text-[18px]">close</span>
                  Reject Changes
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
