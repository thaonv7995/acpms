import { useState } from 'react';
import { ChevronDown, ChevronRight, MessageSquare } from 'lucide-react';
import { InlineComment, Comment } from './InlineComment';
import { Button } from '@/components/ui/button';

interface CommentThreadProps {
  comments: Comment[];
  lineNumber: number;
  onDeleteComment?: (commentId: string) => void;
  currentUserId?: string;
  className?: string;
  isCollapsible?: boolean;
  defaultCollapsed?: boolean;
}

/**
 * CommentThread - Display thread of comments for a specific line
 * Supports collapsible threads and reply expansion
 *
 * @example
 * <CommentThread
 *   comments={lineComments}
 *   lineNumber={42}
 *   onDeleteComment={handleDelete}
 *   currentUserId={userId}
 * />
 */
export function CommentThread({
  comments,
  lineNumber,
  onDeleteComment,
  currentUserId,
  className = '',
  isCollapsible = true,
  defaultCollapsed = false,
}: CommentThreadProps) {
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);

  if (comments.length === 0) return null;

  const commentCount = comments.length;

  return (
    <div className={`border-l-2 border-blue-500 pl-2 ${className}`}>
      {/* Thread Header (collapsible) */}
      {isCollapsible && (
        <div className="flex items-center gap-2 mb-2">
          <Button
            onClick={() => setIsCollapsed(!isCollapsed)}
            size="sm"
            variant="ghost"
            className="h-6 px-2 gap-1 text-slate-600 dark:text-slate-400"
          >
            {isCollapsed ? (
              <ChevronRight className="w-3 h-3" />
            ) : (
              <ChevronDown className="w-3 h-3" />
            )}
            <MessageSquare className="w-3 h-3" />
            <span className="text-xs">
              {commentCount} {commentCount === 1 ? 'comment' : 'comments'} on line {lineNumber}
            </span>
          </Button>
        </div>
      )}

      {/* Comments List */}
      {!isCollapsed && (
        <div className="space-y-2">
          {comments.map((comment) => (
            <InlineComment
              key={comment.id}
              comment={comment}
              onDelete={onDeleteComment}
              canDelete={currentUserId === comment.authorId}
            />
          ))}
        </div>
      )}
    </div>
  );
}
