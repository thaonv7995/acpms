// CommentItem - Individual review comment display
import { useState } from 'react';
import type { ReviewComment } from './types';

interface CommentItemProps {
  comment: ReviewComment;
  onResolve?: (commentId: string) => Promise<void>;
  onUnresolve?: (commentId: string) => Promise<void>;
  isResolvingComment?: boolean;
  showFileInfo?: boolean;
  compact?: boolean;
}

/**
 * Displays a single review comment with resolve/unresolve actions.
 *
 * Usage:
 * <CommentItem
 *   comment={comment}
 *   onResolve={resolveComment}
 *   onUnresolve={unresolveComment}
 * />
 */
export function CommentItem({
  comment,
  onResolve,
  onUnresolve,
  isResolvingComment = false,
  showFileInfo = false,
  compact = false,
}: CommentItemProps) {
  const [isResolving, setIsResolving] = useState(false);

  const handleToggleResolved = async () => {
    if (isResolving || isResolvingComment) return;

    setIsResolving(true);
    try {
      if (comment.resolved) {
        await onUnresolve?.(comment.id);
      } else {
        await onResolve?.(comment.id);
      }
    } finally {
      setIsResolving(false);
    }
  };

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffHours / 24);

    if (diffHours < 1) {
      const diffMins = Math.floor(diffMs / (1000 * 60));
      return diffMins < 1 ? 'just now' : `${diffMins}m ago`;
    }
    if (diffHours < 24) {
      return `${diffHours}h ago`;
    }
    if (diffDays < 7) {
      return `${diffDays}d ago`;
    }
    return date.toLocaleDateString();
  };

  const getUserInitials = (name: string) => {
    return name
      .split(' ')
      .map((n) => n[0])
      .join('')
      .toUpperCase()
      .slice(0, 2);
  };

  return (
    <div
      className={`
        group relative
        ${comment.resolved ? 'opacity-60' : ''}
        ${compact ? 'py-2' : 'py-3'}
      `}
    >
      <div className="flex gap-3">
        {/* Avatar */}
        <div className="shrink-0">
          {comment.user_avatar ? (
            <img
              src={comment.user_avatar}
              alt={comment.user_name}
              className="w-8 h-8 rounded-full object-cover"
            />
          ) : (
            <div className="w-8 h-8 rounded-full bg-primary/10 text-primary flex items-center justify-center text-xs font-bold">
              {getUserInitials(comment.user_name)}
            </div>
          )}
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          {/* Header */}
          <div className="flex items-center gap-2 mb-1">
            <span className="font-medium text-sm text-slate-900 dark:text-white truncate">
              {comment.user_name}
            </span>
            <span className="text-xs text-slate-400 dark:text-slate-500">
              {formatDate(comment.created_at)}
            </span>
            {comment.resolved && (
              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-900/30 rounded">
                <span className="material-symbols-outlined text-[12px]">check</span>
                Resolved
              </span>
            )}
          </div>

          {/* File/Line Info */}
          {showFileInfo && (comment.file_path || comment.line_number) && (
            <div className="flex items-center gap-1.5 mb-1.5 text-xs text-slate-500 dark:text-slate-400">
              <span className="material-symbols-outlined text-[14px]">code</span>
              {comment.file_path && (
                <span className="font-mono truncate">{comment.file_path}</span>
              )}
              {comment.line_number && (
                <span className="font-mono text-primary">:L{comment.line_number}</span>
              )}
            </div>
          )}

          {/* Comment Body */}
          <div
            className={`
              text-sm text-slate-700 dark:text-slate-300
              whitespace-pre-wrap break-words
              ${comment.resolved ? 'line-through decoration-slate-400/50' : ''}
            `}
          >
            {comment.content}
          </div>

          {/* Resolved by info */}
          {comment.resolved && comment.resolved_by_name && (
            <p className="mt-1.5 text-xs text-slate-400 dark:text-slate-500">
              Resolved by {comment.resolved_by_name}
            </p>
          )}
        </div>

        {/* Actions */}
        {(onResolve || onUnresolve) && (
          <div className="shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
            <button
              onClick={handleToggleResolved}
              disabled={isResolving || isResolvingComment}
              className={`
                p-1.5 rounded-lg text-xs font-medium
                transition-all
                disabled:opacity-50 disabled:cursor-not-allowed
                ${
                  comment.resolved
                    ? 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800'
                    : 'text-green-600 hover:text-green-700 hover:bg-green-50 dark:hover:bg-green-900/30'
                }
              `}
              title={comment.resolved ? 'Unresolve comment' : 'Resolve comment'}
            >
              {isResolving ? (
                <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-current"></div>
              ) : (
                <span className="material-symbols-outlined text-[18px]">
                  {comment.resolved ? 'undo' : 'check_circle'}
                </span>
              )}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
