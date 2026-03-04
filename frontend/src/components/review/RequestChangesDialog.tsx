// RequestChangesDialog - Dialog for requesting changes with feedback
import { useState, useEffect } from 'react';

interface RequestChangesDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (request: { feedback: string; include_comments: boolean }) => Promise<void>;
  /** Pre-fill feedback when opening (e.g. for merge conflict resolution) */
  initialFeedback?: string;
  isLoading?: boolean;
  taskTitle?: string;
  unresolvedCommentCount?: number;
}

/**
 * Dialog for requesting changes to submitted code.
 * Spawns a new agent attempt with the feedback as context.
 *
 * Usage:
 * <RequestChangesDialog
 *   isOpen={showRequestChangesDialog}
 *   onClose={() => setShowRequestChangesDialog(false)}
 *   onConfirm={handleRequestChanges}
 * />
 */
export function RequestChangesDialog({
  isOpen,
  onClose,
  onConfirm,
  initialFeedback = '',
  isLoading = false,
  taskTitle,
  unresolvedCommentCount = 0,
}: RequestChangesDialogProps) {
  const [feedback, setFeedback] = useState('');
  const [includeComments, setIncludeComments] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen && initialFeedback) {
      setFeedback(initialFeedback);
    }
  }, [isOpen, initialFeedback]);

  if (!isOpen) return null;

  const handleConfirm = async () => {
    const trimmedFeedback = feedback.trim();
    if (!trimmedFeedback && !(includeComments && unresolvedCommentCount > 0)) {
      setError('Please provide feedback or add comments to the code');
      return;
    }

    setError(null);
    try {
      await onConfirm({
        feedback: trimmedFeedback,
        include_comments: includeComments && unresolvedCommentCount > 0,
      });
      setFeedback('');
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to request changes');
    }
  };

  const handleClose = () => {
    if (!isLoading) {
      setFeedback('');
      setError(null);
      onClose();
    }
  };

  const canSubmit = feedback.trim() || (includeComments && unresolvedCommentCount > 0);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={handleClose}
      />
      <div className="relative w-full max-w-lg bg-white dark:bg-[#0d1117] border border-slate-200 dark:border-slate-700 rounded-2xl shadow-2xl overflow-hidden">
        <div className="p-6">
          {/* Header */}
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 rounded-lg bg-yellow-100 dark:bg-yellow-900/30 text-yellow-600">
              <span className="material-symbols-outlined">edit_note</span>
            </div>
            <div>
              <h2 className="text-lg font-bold text-slate-900 dark:text-white">
                Request Changes
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
            The agent will be re-run with your feedback to address the requested changes.
            The current worktree will be preserved as a starting point.
          </p>

          {/* Include Comments Option */}
          {unresolvedCommentCount > 0 && (
            <div className="mb-4 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg">
              <label className="flex items-start gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeComments}
                  onChange={(e) => setIncludeComments(e.target.checked)}
                  disabled={isLoading}
                  className="mt-0.5 w-4 h-4 text-primary border-slate-300 rounded focus:ring-primary"
                />
                <div>
                  <p className="text-sm font-medium text-blue-700 dark:text-blue-300">
                    Include {unresolvedCommentCount} unresolved{' '}
                    {unresolvedCommentCount === 1 ? 'comment' : 'comments'}
                  </p>
                  <p className="text-xs text-blue-600 dark:text-blue-400">
                    The agent will see all unresolved comments as part of its instructions
                  </p>
                </div>
              </label>
            </div>
          )}

          {/* Feedback Input */}
          <div className="mb-4">
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1.5">
              Additional Feedback
              {unresolvedCommentCount === 0 && <span className="text-red-500 ml-1">*</span>}
            </label>
            <textarea
              value={feedback}
              onChange={(e) => setFeedback(e.target.value)}
              placeholder="Describe what changes you'd like the agent to make..."
              disabled={isLoading}
              rows={4}
              className="w-full px-3 py-2 text-sm border border-slate-200 dark:border-slate-700 rounded-lg bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 placeholder-slate-400 focus:ring-2 focus:ring-yellow-500 focus:border-transparent disabled:opacity-50 resize-none"
            />
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              Be specific about what needs to change. The agent will use this as guidance.
            </p>
          </div>

          {/* Error */}
          {error && (
            <div className="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
              <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
            </div>
          )}

          {/* What will happen */}
          <div className="mb-4 p-3 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
            <p className="text-xs font-medium text-slate-600 dark:text-slate-400 mb-2">
              What will happen:
            </p>
            <ul className="text-xs text-slate-500 dark:text-slate-400 space-y-1">
              <li className="flex items-center gap-2">
                <span className="material-symbols-outlined text-[14px] text-yellow-500">arrow_forward</span>
                A new agent attempt will be created
              </li>
              <li className="flex items-center gap-2">
                <span className="material-symbols-outlined text-[14px] text-yellow-500">arrow_forward</span>
                Your feedback will be added to the agent's context
              </li>
              <li className="flex items-center gap-2">
                <span className="material-symbols-outlined text-[14px] text-yellow-500">arrow_forward</span>
                The agent will continue from the current code state
              </li>
            </ul>
          </div>

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
              disabled={isLoading || !canSubmit}
              className="px-5 py-2 text-white text-sm font-bold rounded-lg bg-yellow-600 hover:bg-yellow-700 shadow-lg shadow-yellow-600/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
            >
              {isLoading ? (
                <>
                  <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                  Sending...
                </>
              ) : (
                <>
                  <span className="material-symbols-outlined text-[18px]">send</span>
                  Request Changes
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
