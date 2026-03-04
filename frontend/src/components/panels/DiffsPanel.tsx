// DiffsPanel - Shows code diffs and git operations with real API integration
import type { KanbanTask } from '../../types/project';
import type { TaskAttempt, BranchStatus } from '../../types/task-attempt';
import { useDiffs, useDiffSummary } from '../../hooks/useDiffs';
import { DiffCard } from '../diffs';

interface DiffsPanelProps {
  attempt: TaskAttempt;
  task: KanbanTask;
  branchStatus?: BranchStatus | null;
}

export function DiffsPanel({ attempt, task: _task, branchStatus: propBranchStatus }: DiffsPanelProps) {
  const {
    diffs,
    branchStatus: fetchedBranchStatus,
    isLoading,
    push,
    isPushing,
    createPr,
    isCreatingPr,
  } = useDiffs(attempt.id);

  const summary = useDiffSummary(diffs);
  const branchStatus = propBranchStatus ?? fetchedBranchStatus;

  return (
    <div className="h-full flex flex-col bg-card">
      {/* Top Bar - Summary Stats */}
      <div className="px-4 py-2 border-b border-border shrink-0">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          {isLoading ? (
            <span>Loading...</span>
          ) : diffs.length > 0 ? (
            <>
              <span className="text-card-foreground">
                {summary.total_files} file{summary.total_files !== 1 ? 's' : ''} changed
              </span>
              <span className="text-green-500">+{summary.total_additions}</span>
              <span className="text-red-500">-{summary.total_deletions}</span>
            </>
          ) : (
            <span>No changes</span>
          )}
        </div>
      </div>

      {/* Branch Info & Actions - Single Row */}
      <div className="px-4 py-2 border-b border-border shrink-0 flex items-center justify-between gap-4">
        {/* Branch Comparison - Left Side */}
        <div className="flex items-center gap-2 text-sm flex-1 min-w-0">
          <span className="material-symbols-outlined text-[16px] text-muted-foreground">account_tree</span>
          <span className="font-mono text-card-foreground truncate">
            {attempt.branch || 'feature/task-' + attempt.id.slice(0, 8)}
          </span>
          <span className="material-symbols-outlined text-[14px] text-muted-foreground">arrow_forward</span>
          <span className="material-symbols-outlined text-[16px] text-muted-foreground">account_tree</span>
          <span className="font-mono text-card-foreground">
            {branchStatus?.target_branch_name || 'main'}
          </span>
          {branchStatus && branchStatus.ahead_count > 0 && (
            <span className="text-xs text-green-500 font-medium ml-2 whitespace-nowrap">
              +{branchStatus.ahead_count} commit{branchStatus.ahead_count !== 1 ? 's' : ''} ahead
            </span>
          )}
        </div>

        {/* Git Action Buttons - Right Side */}
        <div className="flex items-center gap-2 shrink-0">
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 bg-green-600 hover:bg-green-700 text-white rounded text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={isPushing}
            onClick={() => push()}
          >
            <span className="material-symbols-outlined text-[16px]">
              {isPushing ? 'sync' : 'merge'}
            </span>
            Merge
          </button>
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={isCreatingPr}
            onClick={() => createPr()}
          >
            <span className="material-symbols-outlined text-[16px]">
              {isCreatingPr ? 'sync' : 'merge'}
            </span>
            Create PR
          </button>
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 bg-orange-600 hover:bg-orange-700 text-white rounded text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            onClick={() => {
              // Rebase functionality - refresh diffs
              window.location.reload();
            }}
          >
            <span className="material-symbols-outlined text-[16px]">call_split</span>
            Rebase
          </button>
        </div>
      </div>

      {/* Diffs List */}
      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-32">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
          </div>
        ) : diffs.length > 0 ? (
          <div className="py-2">
            {/* Project Name Header */}
            {_task.projectName && (
              <div className="px-4 py-2 flex items-center gap-2 text-sm text-card-foreground">
                <span className="material-symbols-outlined text-[16px]">folder</span>
                <span>{_task.projectName}</span>
              </div>
            )}
            {/* File Diffs */}
            {diffs.map((diff) => (
              <DiffCard 
                key={diff.id} 
                diff={diff} 
                projectName={_task.projectName || undefined}
              />
            ))}
          </div>
        ) : (
          <div className="flex items-center justify-center h-full">
            <div className="text-center p-8">
              <span className="material-symbols-outlined text-muted-foreground text-5xl mb-4">
                difference
              </span>
              <p className="text-muted-foreground">No code changes yet</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
