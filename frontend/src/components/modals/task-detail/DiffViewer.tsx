import { useState, useEffect, useRef, useCallback } from 'react';
import { getAttemptDiff, approveAttempt, DiffResponse } from '../../../api/taskAttempts';
import { DiffCard } from '../../task-detail-page/DiffCard';

interface DiffViewerProps {
  attemptId: string;
  taskStatus: string;
  focusFilePath?: string;
  onApproved?: () => void;
}

export function DiffViewer({ attemptId, taskStatus, focusFilePath, onApproved }: DiffViewerProps) {
  const [diffData, setDiffData] = useState<DiffResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [approving, setApproving] = useState(false);
  const [commitMessage, setCommitMessage] = useState('');

  const isInReview = taskStatus?.toLowerCase() === 'in_review' || taskStatus?.toLowerCase() === 'inreview';
  const fileRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const hasScrolledToFocus = useRef(false);

  const setFileRef = useCallback((path: string, el: HTMLDivElement | null) => {
    if (el) {
      fileRefs.current.set(path, el);
    } else {
      fileRefs.current.delete(path);
    }
  }, []);

  useEffect(() => {
    loadDiff();
    hasScrolledToFocus.current = false;
  }, [attemptId]);

  // Scroll to focused file after data loads
  useEffect(() => {
    if (diffData && focusFilePath && !hasScrolledToFocus.current) {
      hasScrolledToFocus.current = true;
      // Use requestAnimationFrame to wait for DOM render
      requestAnimationFrame(() => {
        // Try exact match first, then partial match (file may be relative vs absolute)
        const el = fileRefs.current.get(focusFilePath)
          || Array.from(fileRefs.current.entries()).find(
            ([path]) => path.endsWith(focusFilePath) || focusFilePath.endsWith(path)
          )?.[1];
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'start' });
          // Add a temporary highlight
          el.classList.add('ring-2', 'ring-primary', 'ring-offset-2');
          setTimeout(() => {
            el.classList.remove('ring-2', 'ring-primary', 'ring-offset-2');
          }, 3000);
        }
      });
    }
  }, [diffData, focusFilePath]);

  const loadDiff = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await getAttemptDiff(attemptId);
      setDiffData(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load diff');
    } finally {
      setLoading(false);
    }
  };

  const handleApprove = async () => {
    try {
      setApproving(true);
      await approveAttempt(attemptId, commitMessage || undefined);
      onApproved?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to approve changes');
    } finally {
      setApproving(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
        <span className="ml-3 text-slate-500">Loading diff...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
        <p className="text-red-600 dark:text-red-400 text-sm">{error}</p>
        <button
          onClick={loadDiff}
          className="mt-2 text-sm text-red-600 dark:text-red-400 underline hover:no-underline"
        >
          Retry
        </button>
      </div>
    );
  }

  if (!diffData || !diffData.files || diffData.files.length === 0) {
    return (
      <div className="p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg text-center">
        <span className="material-symbols-outlined text-4xl text-slate-400 mb-2">difference</span>
        <p className="text-slate-500 dark:text-slate-400">No changes to display</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Diff Stats */}
      <div className="flex items-center gap-4 p-3 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-slate-500">description</span>
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
      <div className="space-y-2">
        {diffData.files.map((file, index) => {
          const filePath = file.new_path || file.old_path || '';
          return (
            <div
              key={filePath || index}
              ref={(el) => setFileRef(filePath, el)}
              className="transition-all duration-300"
            >
              <DiffCard
                diff={file}
                defaultExpanded={true}
              />
            </div>
          );
        })}
      </div>

      {/* Approve Section (only show if in_review) */}
      {isInReview && (
        <div className="border-t border-slate-200 dark:border-slate-700 pt-4 space-y-3">
          <div>
            <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">
              Commit Message (optional)
            </label>
            <input
              type="text"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Approved changes for task..."
              className="w-full px-3 py-2 text-sm border border-slate-200 dark:border-slate-700 rounded-lg bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 focus:ring-2 focus:ring-primary focus:border-transparent"
            />
          </div>
          <button
            onClick={handleApprove}
            disabled={approving}
            className="w-full py-2.5 px-4 bg-green-600 hover:bg-green-700 disabled:bg-green-400 text-white text-sm font-bold rounded-lg shadow-lg shadow-green-600/20 flex items-center justify-center gap-2 transition-all active:scale-95"
          >
            {approving ? (
              <>
                <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                Approving...
              </>
            ) : (
              <>
                <span className="material-symbols-outlined text-[20px]">check_circle</span>
                Approve & Push Changes
              </>
            )}
          </button>
        </div>
      )}
    </div>
  );
}
