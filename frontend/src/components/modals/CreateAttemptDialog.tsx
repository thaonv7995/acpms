// CreateAttemptDialog - Dialog for creating a new attempt for a task
import { useState } from 'react';
import { useCreateTaskAttempt } from '../../api/generated/task-attempts/task-attempts';
import type { RepositoryContext } from '../../types/repository';
import {
  getRepositoryAccessSummary,
  isRepositoryReadOnly,
  normalizeRepositoryContext,
} from '../../utils/repositoryAccess';
import { logger } from '@/lib/logger';

interface CreateAttemptDialogProps {
  isOpen: boolean;
  onClose: () => void;
  taskId: string;
  projectId: string;
  taskTitle: string;
  repositoryContext?: RepositoryContext;
  onSuccess: (attemptId: string) => void;
}

export function CreateAttemptDialog({
  isOpen,
  onClose,
  taskId,
  taskTitle,
  repositoryContext,
  onSuccess,
}: CreateAttemptDialogProps) {
  const createAttemptMutation = useCreateTaskAttempt();
  const [submitError, setSubmitError] = useState<string | null>(null);
  const effectiveRepositoryContext = normalizeRepositoryContext(repositoryContext);
  const repositoryReadOnly = Boolean(repositoryContext) && isRepositoryReadOnly(effectiveRepositoryContext);
  const repositorySummary = getRepositoryAccessSummary(effectiveRepositoryContext);

  const handleCreate = async () => {
    if (repositoryReadOnly) return;

    try {
      setSubmitError(null);
      const result = await createAttemptMutation.mutateAsync({ taskId });
      if (result.data?.id) {
        onSuccess(result.data.id);
      }
    } catch (error) {
      logger.error('Failed to create attempt:', error);
      setSubmitError(error instanceof Error ? error.message : 'Failed to create attempt. Please try again.');
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
      <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose} />
      <div className="relative w-full max-w-md bg-white dark:bg-[#0d1117] border border-slate-200 dark:border-slate-700 rounded-2xl shadow-2xl overflow-hidden">
        <div className="p-6">
          {/* Header */}
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 rounded-lg bg-primary/10 text-primary">
              <span className="material-symbols-outlined">smart_toy</span>
            </div>
            <div>
              <h2 className="text-lg font-bold text-slate-900 dark:text-white">
                Start New Attempt
              </h2>
              <p className="text-sm text-slate-500 dark:text-slate-400">
                Create a new agent attempt for this task
              </p>
            </div>
          </div>

          {/* Task Info */}
          <div className="mb-6 p-3 rounded-lg bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700">
            <p className="text-xs text-slate-500 dark:text-slate-400 mb-1">Task</p>
            <p className="text-sm font-medium text-slate-900 dark:text-white truncate">
              {taskTitle}
            </p>
          </div>

          {/* Info */}
          <div className="mb-6 text-sm text-slate-600 dark:text-slate-400">
            <p>
              This will create a new attempt and start the agent to work on the task.
              The agent will analyze the task description and begin working.
            </p>
          </div>

          {repositoryReadOnly && (
            <div className="mb-4 p-3 rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800">
              <p className="text-sm font-semibold text-amber-800 dark:text-amber-200">
                {repositorySummary.title}
              </p>
              <p className="text-xs text-amber-700 dark:text-amber-300 mt-1">
                {repositorySummary.action}
              </p>
            </div>
          )}

          {/* Error */}
          {submitError ? (
            <div className="mb-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
              <p className="text-sm text-red-600 dark:text-red-400">{submitError}</p>
            </div>
          ) : null}

          {/* Actions */}
          <div className="flex justify-end gap-3">
            <button
              onClick={onClose}
              disabled={createAttemptMutation.isPending}
              className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={handleCreate}
              disabled={createAttemptMutation.isPending || repositoryReadOnly}
              className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 transition-all disabled:opacity-50 flex items-center gap-2"
            >
              {createAttemptMutation.isPending ? (
                <>
                  <span className="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent" />
                  Creating...
                </>
              ) : repositoryReadOnly ? (
                <>
                  <span className="material-symbols-outlined text-[18px]">lock</span>
                  Start Blocked
                </>
              ) : (
                <>
                  <span className="material-symbols-outlined text-[18px]">play_arrow</span>
                  Start Agent
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
