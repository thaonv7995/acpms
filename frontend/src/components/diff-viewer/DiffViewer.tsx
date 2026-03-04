/**
 * DiffViewer - Main container for viewing code changes
 *
 * Features:
 * - Back button navigation to Agent Session
 * - File summary with file tree
 * - Branch info with git actions
 * - Diff content area with multiple files
 * - Live refresh while attempt is active
 */

import { memo, useState, useCallback, useEffect, useMemo } from 'react';
import { useLocation, useParams } from 'react-router-dom';
import { clsx } from 'clsx';
import { useDiff } from './useDiff';
import { DiffViewerHeader } from './DiffViewerHeader';
import { FileSummaryCard } from './FileSummaryCard';
import { BranchInfoCard } from './BranchInfoCard';
import { DiffContentArea } from './DiffContentArea';
import { ReviewActions } from '../review/ReviewActions';
import { ReviewCommentThread } from '../review/ReviewCommentThread';
import { useReviewComments } from '../review/useReviewComments';
import type { RequestChangesRequest } from '../review/types';
import type { ViewMode } from './types';

interface DiffViewerProps {
  attemptId: string;
  taskTitle?: string;
  focusFilePath?: string;
  showOnlyFocusedFile?: boolean;
  /** Hide ReviewActions footer (e.g. when embedded in file popup modal) */
  hideReviewActions?: boolean;
  onBack?: () => void;
  onClose?: () => void;
  onActionComplete?: () => void;
  /** Called when approve/merge succeeds, with message from API (e.g. for toast) */
  onApproveSuccess?: (message: string) => void;
  className?: string;
}

export const DiffViewer = memo(function DiffViewer({
  attemptId,
  taskTitle,
  focusFilePath,
  showOnlyFocusedFile = false,
  hideReviewActions = false,
  onBack,
  onClose,
  onActionComplete,
  onApproveSuccess,
  className,
}: DiffViewerProps) {
  const [selectedFile, setSelectedFile] = useState<string | undefined>();
  const [defaultViewMode] = useState<ViewMode>('side-by-side');
  const [showComments, setShowComments] = useState(true);
  const [areAllFilesExpanded, setAreAllFilesExpanded] = useState(true);
  const [expandAllSignal, setExpandAllSignal] = useState(0);
  const location = useLocation();
  const { projectId, taskId } = useParams<{ projectId?: string; taskId?: string }>();

  const { files, summary, branchInfo, availableActions, isLoading, error, refresh } = useDiff({
    attemptId,
    enabled: true,
    realtime: true,
  });

  // Review comments and actions
  const {
    commentsByFile,
    reviewStatus,
    approve,
    reject,
    requestChanges,
    isApproving,
    isRejecting,
    isRequestingChanges,
    addComment,
    resolveComment,
    unresolveComment,
  } = useReviewComments(attemptId);

  const handleApprove = useCallback(async (commitMessage?: string) => {
    const message = await approve({ commit_message: commitMessage });
    onApproveSuccess?.(message);
    refresh();
    onActionComplete?.();
  }, [approve, onApproveSuccess, refresh, onActionComplete]);

  const handleReject = useCallback(async (reason: string) => {
    await reject({ reason });
    onActionComplete?.();
  }, [reject, onActionComplete]);

  const handleRequestChanges = useCallback(async (request: RequestChangesRequest) => {
    await requestChanges(request);
    onActionComplete?.();
  }, [requestChanges, onActionComplete]);

  const handleMerge = useCallback(async () => {
    await handleApprove();
  }, [handleApprove]);

  const handleCreatePR = useCallback(async () => {
    await handleApprove();
  }, [handleApprove]);

  const handleRebase = useCallback(() => {
    refresh();
  }, [refresh]);

  const handleFileSelect = useCallback((path: string) => {
    setSelectedFile(path);
  }, []);

  const handleToggleExpandAll = useCallback(() => {
    setAreAllFilesExpanded((prev) => !prev);
    setExpandAllSignal((prev) => prev + 1);
  }, []);

  const canReviewActions = Boolean(availableActions?.canMerge || availableActions?.canReject);

  const buildAttemptPath = useCallback(() => {
    if (projectId && taskId) {
      if (location.pathname.startsWith('/projects/')) {
        return `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}`;
      }
      return `/tasks/projects/${projectId}/${taskId}/attempts/${attemptId}`;
    }

    if (location.pathname.includes('/attempts/')) {
      return location.pathname.replace(/\/attempts\/[^/]+/, `/attempts/${attemptId}`);
    }

    return location.pathname;
  }, [attemptId, location.pathname, projectId, taskId]);

  const handleOpenInNew = useCallback(() => {
    const targetPath = buildAttemptPath();
    window.open(`${window.location.origin}${targetPath}`, '_blank', 'noopener,noreferrer');
  }, [buildAttemptPath]);

  const resolvedFocusPath = useMemo(() => {
    if (!focusFilePath || files.length === 0) return undefined;
    const directMatch = files.find(
      (file) => file.path === focusFilePath || file.oldPath === focusFilePath
    );
    const suffixMatch = files.find(
      (file) =>
        file.path.endsWith(focusFilePath) ||
        focusFilePath.endsWith(file.path) ||
        (file.oldPath ? file.oldPath.endsWith(focusFilePath) : false)
    );
    return (directMatch ?? suffixMatch)?.path;
  }, [focusFilePath, files]);

  useEffect(() => {
    if (!resolvedFocusPath) return;
    setSelectedFile(resolvedFocusPath);
  }, [resolvedFocusPath]);

  const visibleFiles = useMemo(() => {
    if (!showOnlyFocusedFile || !resolvedFocusPath) {
      return files;
    }
    return files.filter((file) => file.path === resolvedFocusPath);
  }, [files, showOnlyFocusedFile, resolvedFocusPath]);

  const visibleSummary = useMemo(() => {
    if (!showOnlyFocusedFile || !resolvedFocusPath) {
      return summary;
    }

    const focused = visibleFiles[0];
    if (!focused) return summary;

    return {
      totalFiles: 1,
      totalAdditions: focused.additions,
      totalDeletions: focused.deletions,
      filesAdded: focused.status === 'added' ? 1 : 0,
      filesModified: focused.status === 'modified' ? 1 : 0,
      filesDeleted: focused.status === 'deleted' ? 1 : 0,
      filesRenamed: focused.status === 'renamed' ? 1 : 0,
    };
  }, [summary, showOnlyFocusedFile, resolvedFocusPath, visibleFiles]);

  const showExpandToggle = visibleFiles.length > 0 && !isLoading;

  return (
    <div className={clsx('h-full flex flex-col bg-background text-foreground', className)}>
      <DiffViewerHeader
        attemptId={attemptId}
        taskTitle={taskTitle}
        isLoading={isLoading}
        showExpandToggle={showExpandToggle}
        areAllFilesExpanded={areAllFilesExpanded}
        onBack={onBack}
        onClose={onClose}
        onRefresh={refresh}
        onOpenInNew={handleOpenInNew}
        onToggleExpandAll={handleToggleExpandAll}
      />

      {/* Content */}
      <div className="flex-1 overflow-auto px-3 py-3 space-y-3">
        {/* Error state */}
        {error && (
          <div className="px-3 py-2 bg-destructive/10 border border-destructive/30 rounded-sm text-sm text-destructive">
            <div className="flex items-center gap-2">
              <span className="material-symbols-outlined text-[16px]">error</span>
              <span>{error}</span>
            </div>
            <button onClick={refresh} className="mt-1 text-xs underline hover:no-underline">
              Try again
            </button>
          </div>
        )}

        {/* Loading state */}
        {isLoading && files.length === 0 && (
          <div className="flex items-center justify-center py-12">
            <div className="text-center">
              <span className="material-symbols-outlined text-4xl text-muted-foreground animate-spin">
                progress_activity
              </span>
              <p className="mt-2 text-sm text-muted-foreground">Loading diff...</p>
            </div>
          </div>
        )}

        {/* File Summary Card */}
        {!isLoading && !showOnlyFocusedFile && (
          <FileSummaryCard
            summary={visibleSummary}
            files={visibleFiles}
            selectedFile={selectedFile}
            onFileSelect={handleFileSelect}
          />
        )}

        {/* Branch Info Card
            Hide in single-file modal mode: this mode is for quick file inspection only. */}
        {!isLoading && !showOnlyFocusedFile && (
          <BranchInfoCard
            branchInfo={branchInfo}
            availableActions={availableActions}
            attemptId={attemptId}
            onMerge={handleMerge}
            onCreatePR={handleCreatePR}
            onRebase={handleRebase}
            isLoading={isLoading}
          />
        )}

        {/* Diff Content Area */}
        {!isLoading && visibleFiles.length > 0 && (
          <DiffContentArea
            files={visibleFiles}
            selectedFile={selectedFile}
            defaultViewMode={defaultViewMode}
            onFileSelect={handleFileSelect}
            attemptId={attemptId}
            expandAllSignal={expandAllSignal}
            forceExpanded={areAllFilesExpanded}
            onAddComment={addComment}
          />
        )}

        {/* Review Comments Panel */}
        {!isLoading && showComments && commentsByFile.length > 0 && (
          <div className="border border-border overflow-hidden">
            <div className="flex items-center justify-between px-3 py-2 bg-muted/30 border-b border-border">
              <h3 className="text-sm font-medium text-foreground flex items-center gap-2">
                <span className="material-symbols-outlined text-[16px]">comment</span>
                Review Comments ({reviewStatus.totalComments})
              </h3>
              <button
                onClick={() => setShowComments(false)}
                className="h-6 w-6 inline-flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-muted/50 rounded-sm"
              >
                <span className="material-symbols-outlined text-[16px]">close</span>
              </button>
            </div>
            <div className="max-h-[300px] overflow-auto">
              <ReviewCommentThread
                commentsByFile={commentsByFile}
                reviewStatus={reviewStatus}
                onAddComment={async (content, filePath, lineNumber) => {
                  await addComment({ content, file_path: filePath, line_number: lineNumber });
                }}
                onResolveComment={resolveComment}
                onUnresolveComment={unresolveComment}
              />
            </div>
          </div>
        )}
      </div>

      {/* Footer with ReviewActions (hidden in popup/modal) */}
      {!hideReviewActions && !isLoading && visibleFiles.length > 0 && canReviewActions && (
        <ReviewActions
          attemptId={attemptId}
          taskTitle={taskTitle}
          reviewStatus={reviewStatus}
          onApprove={handleApprove}
          onReject={handleReject}
          onRequestChanges={handleRequestChanges}
          isApproving={isApproving}
          isRejecting={isRejecting}
          isRequestingChanges={isRequestingChanges}
        />
      )}
    </div>
  );
});
