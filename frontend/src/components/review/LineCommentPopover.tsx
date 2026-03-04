// LineCommentPopover - Popover for adding comments on diff lines
import { useState, useEffect, useRef } from 'react';
import type { ReviewComment, LineCommentPosition } from './types';
import { AddCommentForm } from './AddCommentForm';
import { CommentItem } from './CommentItem';

interface LineCommentPopoverProps {
  position: LineCommentPosition | null;
  existingComments: ReviewComment[];
  onAddComment: (content: string, filePath: string, lineNumber: number) => Promise<void>;
  onResolveComment: (commentId: string) => Promise<void>;
  onUnresolveComment: (commentId: string) => Promise<void>;
  onClose: () => void;
  isAddingComment?: boolean;
  isResolvingComment?: boolean;
}

/**
 * Popover that appears when clicking on a diff line.
 * Shows existing comments and allows adding new ones.
 *
 * Usage:
 * <LineCommentPopover
 *   position={lineCommentPosition}
 *   existingComments={getLineComments(filePath, lineNumber)}
 *   onAddComment={addComment}
 *   onClose={() => setLineCommentPosition(null)}
 * />
 */
export function LineCommentPopover({
  position,
  existingComments,
  onAddComment,
  onResolveComment,
  onUnresolveComment,
  onClose,
  isAddingComment = false,
  isResolvingComment = false,
}: LineCommentPopoverProps) {
  const popoverRef = useRef<HTMLDivElement>(null);
  const [showAddForm, setShowAddForm] = useState(existingComments.length === 0);

  // Close on escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  // Close on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  // Reset state when position changes
  useEffect(() => {
    setShowAddForm(existingComments.length === 0);
  }, [position, existingComments.length]);

  if (!position) return null;

  const handleAddComment = async (content: string) => {
    await onAddComment(content, position.filePath, position.lineNumber);
    setShowAddForm(false);
  };

  // Calculate position styles
  const getPositionStyle = () => {
    const popoverWidth = 400;
    const popoverMaxHeight = 400;
    const padding = 16;

    let left = position.x;
    let top = position.y + 24; // Below the line

    // Adjust if going off screen right
    if (left + popoverWidth > window.innerWidth - padding) {
      left = window.innerWidth - popoverWidth - padding;
    }

    // Adjust if going off screen left
    if (left < padding) {
      left = padding;
    }

    // Adjust if going off screen bottom
    if (top + popoverMaxHeight > window.innerHeight - padding) {
      top = position.y - popoverMaxHeight - 8; // Above the line
    }

    return {
      position: 'fixed' as const,
      left: `${left}px`,
      top: `${top}px`,
      width: `${popoverWidth}px`,
      maxHeight: `${popoverMaxHeight}px`,
    };
  };

  const lineTypeLabel = {
    add: 'added',
    delete: 'deleted',
    normal: '',
  }[position.lineType];

  const lineTypeColor = {
    add: 'text-green-600 bg-green-50 dark:bg-green-900/30',
    delete: 'text-red-600 bg-red-50 dark:bg-red-900/30',
    normal: 'text-slate-600 bg-slate-50 dark:bg-slate-800/50',
  }[position.lineType];

  return (
    <div
      ref={popoverRef}
      style={getPositionStyle()}
      className="z-50 bg-white dark:bg-[#0d1117] border border-slate-200 dark:border-slate-700 rounded-xl shadow-2xl overflow-hidden flex flex-col"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-slate-50 dark:bg-slate-800/50 border-b border-slate-200 dark:border-slate-700">
        <div className="flex items-center gap-2 min-w-0">
          <span className="material-symbols-outlined text-[18px] text-slate-500">comment</span>
          <span className="text-sm font-medium text-slate-700 dark:text-slate-300 truncate">
            Line {position.lineNumber}
          </span>
          {lineTypeLabel && (
            <span className={`px-1.5 py-0.5 text-[10px] font-medium rounded ${lineTypeColor}`}>
              {lineTypeLabel}
            </span>
          )}
        </div>
        <button
          onClick={onClose}
          className="p-1 text-slate-400 hover:text-slate-600 dark:hover:text-white rounded transition-colors"
        >
          <span className="material-symbols-outlined text-[18px]">close</span>
        </button>
      </div>

      {/* File path */}
      <div className="px-4 py-1.5 border-b border-slate-100 dark:border-slate-800">
        <p className="text-xs font-mono text-slate-500 dark:text-slate-400 truncate">
          {position.filePath}
        </p>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {/* Existing Comments */}
        {existingComments.length > 0 && (
          <div className="divide-y divide-slate-100 dark:divide-slate-800">
            {existingComments.map((comment) => (
              <div key={comment.id} className="px-4">
                <CommentItem
                  comment={comment}
                  onResolve={onResolveComment}
                  onUnresolve={onUnresolveComment}
                  isResolvingComment={isResolvingComment}
                  compact
                />
              </div>
            ))}
          </div>
        )}

        {/* Add Comment Section */}
        {showAddForm ? (
          <div className="p-4 border-t border-slate-100 dark:border-slate-800">
            <AddCommentForm
              onSubmit={handleAddComment}
              onCancel={existingComments.length > 0 ? () => setShowAddForm(false) : undefined}
              isSubmitting={isAddingComment}
              placeholder={`Comment on line ${position.lineNumber}...`}
              autoFocus
              showCancelButton={existingComments.length > 0}
              compact
            />
          </div>
        ) : (
          <div className="p-3 border-t border-slate-100 dark:border-slate-800">
            <button
              onClick={() => setShowAddForm(true)}
              className="w-full flex items-center justify-center gap-2 py-2 text-xs font-medium text-primary hover:text-blue-600 hover:bg-primary/5 rounded-lg transition-colors"
            >
              <span className="material-symbols-outlined text-[16px]">add</span>
              Add Comment
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
