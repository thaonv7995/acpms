import { useState, useEffect } from 'react';
import { getAttemptDiff, approveAttempt, DiffResponse, getAttempt, TaskAttempt } from '../../api/taskAttempts';
import { DiffCard } from '../task-detail-page/DiffCard';

interface ReviewChangesModalProps {
  isOpen: boolean;
  onClose: () => void;
  attemptId: string;
  taskTitle?: string;
  projectName?: string;
  onApproved?: () => void;
}

export function ReviewChangesModal({
  isOpen,
  onClose,
  attemptId,
  taskTitle,
  projectName,
  onApproved,
}: ReviewChangesModalProps) {
  const [diffData, setDiffData] = useState<DiffResponse | null>(null);
  const [attempt, setAttempt] = useState<TaskAttempt | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [approving, setApproving] = useState(false);
  const [commitMessage, setCommitMessage] = useState('');
  const [activeTab, setActiveTab] = useState<'diff' | 'info'>('diff');

  useEffect(() => {
    if (isOpen && attemptId) {
      loadData();
    }
  }, [isOpen, attemptId]);

  const loadData = async () => {
    try {
      setLoading(true);
      setError(null);

      // Load attempt details and diff in parallel
      const [attemptData, diffData] = await Promise.all([
        getAttempt(attemptId).catch(() => null),
        getAttemptDiff(attemptId).catch(() => null),
      ]);

      setAttempt(attemptData);
      setDiffData(diffData);

      if (!diffData) {
        setError('No diff available. The worktree may have been cleaned up.');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load data');
    } finally {
      setLoading(false);
    }
  };

  const handleApprove = async () => {
    try {
      setApproving(true);
      setError(null);
      await approveAttempt(attemptId, commitMessage || undefined);
      onApproved?.();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to approve changes');
    } finally {
      setApproving(false);
    }
  };

  if (!isOpen) return null;

  // Check if task needs review (worktree_path exists in metadata means it's awaiting review)
  const hasWorktreePath = Boolean(attempt?.metadata?.worktree_path);
  const needsReview = hasWorktreePath && attempt?.status === 'SUCCESS';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={onClose}
      />
      <div className="relative w-full max-w-4xl bg-white dark:bg-[#0d1117] border border-slate-200 dark:border-slate-700 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex justify-between items-center bg-slate-50 dark:bg-[#161b22]">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-primary">difference</span>
            <div>
              <h2 className="text-lg font-bold text-slate-900 dark:text-white">
                Review Changes
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                {projectName && <span>{projectName}</span>}
                {projectName && taskTitle && <span> / </span>}
                {taskTitle && <span>{taskTitle}</span>}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-slate-600 dark:hover:text-white transition-colors rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800"
          >
            <span className="material-symbols-outlined text-[20px]">close</span>
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="px-6 pt-4 border-b border-slate-200 dark:border-slate-700">
          <div className="flex gap-4">
            <button
              onClick={() => setActiveTab('diff')}
              className={`pb-3 px-1 text-sm font-medium border-b-2 transition-colors ${
                activeTab === 'diff'
                  ? 'border-primary text-primary'
                  : 'border-transparent text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              <span className="flex items-center gap-2">
                <span className="material-symbols-outlined text-[18px]">difference</span>
                Changes
              </span>
            </button>
            <button
              onClick={() => setActiveTab('info')}
              className={`pb-3 px-1 text-sm font-medium border-b-2 transition-colors ${
                activeTab === 'info'
                  ? 'border-primary text-primary'
                  : 'border-transparent text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
              }`}
            >
              <span className="flex items-center gap-2">
                <span className="material-symbols-outlined text-[18px]">info</span>
                Details
              </span>
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-6">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
              <span className="ml-3 text-slate-500">Loading...</span>
            </div>
          ) : error && !diffData ? (
            <div className="p-4 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg">
              <p className="text-yellow-700 dark:text-yellow-400 text-sm">{error}</p>
              <button
                onClick={loadData}
                className="mt-2 text-sm text-yellow-700 dark:text-yellow-400 underline hover:no-underline"
              >
                Retry
              </button>
            </div>
          ) : activeTab === 'diff' ? (
            <div className="space-y-4">
              {/* Diff Stats */}
              {diffData && (
                <>
                  <div className="flex items-center gap-4 p-3 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-slate-500 text-[18px]">description</span>
                      <span className="text-sm text-slate-600 dark:text-slate-300">
                        {diffData.total_files} file{diffData.total_files !== 1 ? 's' : ''} changed
                      </span>
                    </div>
                    <div className="flex items-center gap-2 text-green-600 dark:text-green-400">
                      <span className="text-sm font-mono">+{diffData.total_additions}</span>
                    </div>
                    <div className="flex items-center gap-2 text-red-600 dark:text-red-400">
                      <span className="text-sm font-mono">-{diffData.total_deletions}</span>
                    </div>
                  </div>

                  {/* Diff Content */}
                  {diffData.files && diffData.files.length > 0 ? (
                    <div className="space-y-2">
                      {diffData.files.map((file, index) => (
                        <DiffCard
                          key={file.new_path || file.old_path || index}
                          diff={file}
                          defaultExpanded={true}
                        />
                      ))}
                    </div>
                  ) : (
                    <div className="p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg text-center">
                      <span className="material-symbols-outlined text-4xl text-slate-400 mb-2">check_circle</span>
                      <p className="text-slate-500 dark:text-slate-400">No uncommitted changes</p>
                    </div>
                  )}
                </>
              )}
            </div>
          ) : (
            <div className="space-y-4">
              {/* Attempt Details */}
              <div className="grid grid-cols-2 gap-4">
                <div className="p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
                  <p className="text-xs text-slate-500 dark:text-slate-400 mb-1">Status</p>
                  <p className="text-sm font-medium text-slate-900 dark:text-white">
                    {attempt?.status || 'Unknown'}
                  </p>
                </div>
                <div className="p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
                  <p className="text-xs text-slate-500 dark:text-slate-400 mb-1">Attempt ID</p>
                  <p className="text-sm font-mono text-slate-900 dark:text-white truncate" title={attemptId}>
                    {attemptId.substring(0, 8)}...
                  </p>
                </div>
                <div className="p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
                  <p className="text-xs text-slate-500 dark:text-slate-400 mb-1">Started</p>
                  <p className="text-sm text-slate-900 dark:text-white">
                    {attempt?.started_at
                      ? new Date(attempt.started_at).toLocaleString()
                      : 'Not started'}
                  </p>
                </div>
                <div className="p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
                  <p className="text-xs text-slate-500 dark:text-slate-400 mb-1">Completed</p>
                  <p className="text-sm text-slate-900 dark:text-white">
                    {attempt?.completed_at
                      ? new Date(attempt.completed_at).toLocaleString()
                      : 'In progress'}
                  </p>
                </div>
              </div>
              {attempt?.error_message && (
                <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                  <p className="text-xs text-red-500 dark:text-red-400 mb-1">Error</p>
                  <p className="text-sm text-red-700 dark:text-red-300">{attempt.error_message}</p>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Footer with Approve Button */}
        {needsReview && diffData?.files && diffData.files.length > 0 && (
          <div className="px-6 py-4 border-t border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-[#161b22]">
            {error && (
              <p className="text-red-500 text-sm mb-3">{error}</p>
            )}
            <div className="flex items-center gap-4">
              <div className="flex-1">
                <input
                  type="text"
                  value={commitMessage}
                  onChange={(e) => setCommitMessage(e.target.value)}
                  placeholder="Commit message (optional)"
                  className="w-full px-3 py-2 text-sm border border-slate-200 dark:border-slate-700 rounded-lg bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 focus:ring-2 focus:ring-primary focus:border-transparent"
                />
              </div>
              <button
                onClick={handleApprove}
                disabled={approving}
                className="py-2.5 px-6 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-bold rounded-lg shadow-lg shadow-green-600/20 flex items-center gap-2 transition-all active:scale-95"
              >
                {approving ? (
                  <>
                    <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                    Approving...
                  </>
                ) : (
                  <>
                    <span className="material-symbols-outlined text-[18px]">check_circle</span>
                    Approve & Push
                  </>
                )}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
