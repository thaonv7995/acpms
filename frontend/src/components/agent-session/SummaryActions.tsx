/**
 * SummaryActions - Summary card with action buttons
 * Shows file changes summary and quick actions
 */

import { memo, useState } from 'react';
import type { DiffSummary } from './types';

interface SummaryActionsProps {
  summary: DiffSummary;
  status: 'idle' | 'running' | 'completed' | 'failed' | 'cancelled' | 'waiting_input';
  onViewChanges?: () => void;
  onCopyOutput?: () => void;
  onRestart?: () => void;
  onCancel?: () => void;
  onContinue?: () => void;
  className?: string;
}

export const SummaryActions = memo(function SummaryActions({
  summary,
  status,
  onViewChanges,
  onCopyOutput,
  onRestart,
  onCancel,
  onContinue,
  className = '',
}: SummaryActionsProps) {
  const [copied, setCopied] = useState(false);
  const isRunning = status === 'running';
  const isCompleted = status === 'completed';
  const isFailed = status === 'failed';
  const isWaitingInput = status === 'waiting_input';

  const handleCopy = () => {
    onCopyOutput?.();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className={`bg-slate-800/50 dark:bg-slate-800/70 rounded-lg border border-slate-700/50 ${className}`}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-700/50">
        <h4 className="text-sm font-semibold text-slate-200">Summary & Actions</h4>
        <div className="flex items-center gap-1">
          {/* Copy output */}
          <button
            onClick={handleCopy}
            className="p-1.5 rounded hover:bg-slate-700/50 text-slate-400 hover:text-slate-300 transition-colors"
            title="Copy output"
          >
            <span className="material-symbols-outlined text-[16px]">
              {copied ? 'check' : 'content_copy'}
            </span>
          </button>

          {/* View files */}
          {summary.filesChanged > 0 && (
            <button
              onClick={onViewChanges}
              className="p-1.5 rounded hover:bg-slate-700/50 text-slate-400 hover:text-slate-300 transition-colors"
              title="View changed files"
            >
              <span className="material-symbols-outlined text-[16px]">folder_open</span>
            </button>
          )}
        </div>
      </div>

      {/* Stats */}
      <div className="px-4 py-3">
        <div className="flex items-center gap-4 text-sm">
          {/* Files changed */}
          <div className="flex items-center gap-1.5 text-slate-300">
            <span className="material-symbols-outlined text-[16px] text-slate-500">
              description
            </span>
            <span className="font-medium">{summary.filesChanged}</span>
            <span className="text-slate-500">
              {summary.filesChanged === 1 ? 'file' : 'files'} changed
            </span>
          </div>

          {/* Additions */}
          {summary.additions > 0 && (
            <span className="text-green-400 font-mono">+{summary.additions}</span>
          )}

          {/* Deletions */}
          {summary.deletions > 0 && (
            <span className="text-red-400 font-mono">-{summary.deletions}</span>
          )}
        </div>

        {/* Detailed breakdown */}
        {(summary.filesAdded || summary.filesModified || summary.filesDeleted) && (
          <div className="flex items-center gap-3 mt-2 text-xs text-slate-500">
            {summary.filesAdded && summary.filesAdded > 0 && (
              <span className="flex items-center gap-1">
                <span className="size-2 rounded-full bg-green-500" />
                {summary.filesAdded} added
              </span>
            )}
            {summary.filesModified && summary.filesModified > 0 && (
              <span className="flex items-center gap-1">
                <span className="size-2 rounded-full bg-amber-500" />
                {summary.filesModified} modified
              </span>
            )}
            {summary.filesDeleted && summary.filesDeleted > 0 && (
              <span className="flex items-center gap-1">
                <span className="size-2 rounded-full bg-red-500" />
                {summary.filesDeleted} deleted
              </span>
            )}
          </div>
        )}
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-2 px-4 py-3 border-t border-slate-700/50">
        {/* View Changes button - primary action */}
        {(isCompleted || isFailed) && summary.filesChanged > 0 && (
          <button
            onClick={onViewChanges}
            className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg text-sm font-medium transition-colors"
          >
            <span className="material-symbols-outlined text-[18px]">difference</span>
            View Changes
          </button>
        )}

        {/* Running status */}
        {isRunning && (
          <>
            <div className="flex-1 flex items-center gap-2 px-4 py-2 bg-slate-700/50 rounded-lg text-sm text-slate-300">
              <span className="size-2 bg-blue-500 rounded-full animate-pulse" />
              Agent is working...
            </div>
            <button
              onClick={onCancel}
              className="px-4 py-2 bg-red-500/20 hover:bg-red-500/30 text-red-400 rounded-lg text-sm font-medium transition-colors"
            >
              Cancel
            </button>
          </>
        )}

        {/* Waiting for input */}
        {isWaitingInput && (
          <div className="flex-1 flex items-center gap-2 px-4 py-2 bg-amber-500/20 rounded-lg text-sm text-amber-400">
            <span className="material-symbols-outlined text-[18px]">edit</span>
            Waiting for your input...
          </div>
        )}

        {/* Completed/Failed actions */}
        {(isCompleted || isFailed) && (
          <>
            {summary.filesChanged === 0 && (
              <div className="flex-1 flex items-center gap-2 px-4 py-2 bg-slate-700/50 rounded-lg text-sm text-slate-400">
                <span className="material-symbols-outlined text-[18px]">info</span>
                No file changes
              </div>
            )}

            {/* Restart button */}
            <button
              onClick={onRestart}
              className="p-2 bg-slate-700/50 hover:bg-slate-700 rounded-lg text-slate-400 hover:text-slate-300 transition-colors"
              title="Restart task"
            >
              <span className="material-symbols-outlined text-[18px]">refresh</span>
            </button>
          </>
        )}

        {/* Continue button for waiting_input */}
        {isWaitingInput && onContinue && (
          <button
            onClick={onContinue}
            className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg text-sm font-medium transition-colors"
          >
            Continue
          </button>
        )}
      </div>
    </div>
  );
});
