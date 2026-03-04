// FileCommentGroup - Collapsible group of comments for a file
import type { ReviewComment } from './types';
import { CommentItem } from './CommentItem';

interface FileCommentGroupProps {
  filePath: string | null;
  comments: ReviewComment[];
  isExpanded: boolean;
  onToggle: () => void;
  onResolveComment: (commentId: string) => Promise<void>;
  onUnresolveComment: (commentId: string) => Promise<void>;
  isResolvingComment?: boolean;
}

/**
 * Collapsible group of comments for a specific file or general comments.
 * Used within ReviewCommentThread to organize comments by file.
 */
export function FileCommentGroup({
  filePath,
  comments,
  isExpanded,
  onToggle,
  onResolveComment,
  onUnresolveComment,
  isResolvingComment,
}: FileCommentGroupProps) {
  const resolvedCount = comments.filter((c) => c.resolved).length;
  const isGeneral = filePath === null;

  return (
    <div className="border border-slate-200 dark:border-slate-700 rounded-lg overflow-hidden">
      {/* Group Header */}
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-4 py-2.5 bg-slate-50 dark:bg-slate-800/50 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors text-left"
      >
        <span
          className={`material-symbols-outlined text-[16px] text-slate-400 transition-transform ${
            isExpanded ? 'rotate-90' : ''
          }`}
        >
          chevron_right
        </span>
        {isGeneral ? (
          <>
            <span className="material-symbols-outlined text-[16px] text-slate-500">forum</span>
            <span className="text-sm font-medium text-slate-700 dark:text-slate-300">
              General Comments
            </span>
          </>
        ) : (
          <>
            <span className="material-symbols-outlined text-[16px] text-slate-500">code</span>
            <span className="text-sm font-mono text-slate-700 dark:text-slate-300 truncate flex-1">
              {filePath}
            </span>
          </>
        )}
        <span className="text-xs text-slate-500 dark:text-slate-400 shrink-0">
          {resolvedCount}/{comments.length}
        </span>
      </button>

      {/* Comments List */}
      {isExpanded && (
        <div className="divide-y divide-slate-100 dark:divide-slate-800">
          {comments.map((comment) => (
            <div key={comment.id} className="px-4">
              <CommentItem
                comment={comment}
                onResolve={onResolveComment}
                onUnresolve={onUnresolveComment}
                isResolvingComment={isResolvingComment}
                showFileInfo={!isGeneral && comment.line_number !== null}
                compact
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
