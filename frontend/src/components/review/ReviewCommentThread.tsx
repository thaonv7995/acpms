// ReviewCommentThread - Threaded comments display grouped by file
import { useState } from 'react';
import type { CommentsByFile, ReviewStatus } from './types';
import { FileCommentGroup } from './FileCommentGroup';
import { AddCommentForm } from './AddCommentForm';

interface ReviewCommentThreadProps {
  commentsByFile: CommentsByFile[];
  reviewStatus: ReviewStatus;
  onAddComment: (content: string, filePath?: string, lineNumber?: number) => Promise<void>;
  onResolveComment: (commentId: string) => Promise<void>;
  onUnresolveComment: (commentId: string) => Promise<void>;
  isAddingComment?: boolean;
  isResolvingComment?: boolean;
  isLoading?: boolean;
}

/**
 * Displays review comments grouped by file with add comment form.
 * Shows general comments first, then file-specific comments.
 *
 * Usage:
 * <ReviewCommentThread
 *   commentsByFile={commentsByFile}
 *   reviewStatus={reviewStatus}
 *   onAddComment={addComment}
 *   onResolveComment={resolveComment}
 *   onUnresolveComment={unresolveComment}
 * />
 */
export function ReviewCommentThread({
  commentsByFile,
  reviewStatus,
  onAddComment,
  onResolveComment,
  onUnresolveComment,
  isAddingComment = false,
  isResolvingComment = false,
  isLoading = false,
}: ReviewCommentThreadProps) {
  const [showAddForm, setShowAddForm] = useState(false);
  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set(['general']));

  const toggleFile = (filePath: string) => {
    setExpandedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(filePath)) {
        next.delete(filePath);
      } else {
        next.add(filePath);
      }
      return next;
    });
  };

  const handleAddGeneralComment = async (content: string) => {
    await onAddComment(content);
    setShowAddForm(false);
  };

  if (isLoading) {
    return (
      <div className="p-4 flex items-center justify-center">
        <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary"></div>
        <span className="ml-2 text-sm text-slate-500">Loading comments...</span>
      </div>
    );
  }

  const totalComments = reviewStatus.totalComments;
  const hasComments = totalComments > 0;

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 bg-slate-50 dark:bg-slate-800/50 rounded-lg">
        <div className="flex items-center gap-3">
          <span className="material-symbols-outlined text-slate-500 text-[20px]">comment</span>
          <span className="text-sm font-medium text-slate-700 dark:text-slate-300">
            Comments
            {hasComments && (
              <span className="ml-1.5 text-slate-500">
                ({reviewStatus.resolvedComments}/{totalComments} resolved)
              </span>
            )}
          </span>
        </div>
        {!showAddForm && (
          <button
            onClick={() => setShowAddForm(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-primary hover:text-blue-600 hover:bg-primary/5 rounded-lg transition-colors"
          >
            <span className="material-symbols-outlined text-[16px]">add</span>
            Add Comment
          </button>
        )}
      </div>

      {/* Add General Comment Form */}
      {showAddForm && (
        <div className="px-4">
          <div className="p-3 bg-slate-50 dark:bg-slate-800/50 rounded-lg border border-slate-200 dark:border-slate-700">
            <p className="text-xs text-slate-500 dark:text-slate-400 mb-2">
              Add a general review comment
            </p>
            <AddCommentForm
              onSubmit={handleAddGeneralComment}
              onCancel={() => setShowAddForm(false)}
              isSubmitting={isAddingComment}
              placeholder="Write a general comment about this review..."
              autoFocus
            />
          </div>
        </div>
      )}

      {/* Comments List */}
      {hasComments ? (
        <div className="space-y-2">
          {commentsByFile.map((group) => (
            <FileCommentGroup
              key={group.filePath ?? 'general'}
              filePath={group.filePath}
              comments={group.comments}
              isExpanded={expandedFiles.has(group.filePath ?? 'general')}
              onToggle={() => toggleFile(group.filePath ?? 'general')}
              onResolveComment={onResolveComment}
              onUnresolveComment={onUnresolveComment}
              isResolvingComment={isResolvingComment}
            />
          ))}
        </div>
      ) : !showAddForm ? (
        <div className="py-8 text-center">
          <span className="material-symbols-outlined text-4xl text-slate-300 dark:text-slate-600 mb-2 block">
            chat_bubble_outline
          </span>
          <p className="text-sm text-slate-500 dark:text-slate-400">
            No comments yet
          </p>
          <button
            onClick={() => setShowAddForm(true)}
            className="mt-2 text-sm text-primary hover:text-blue-600 font-medium"
          >
            Be the first to comment
          </button>
        </div>
      ) : null}
    </div>
  );
}
