import { formatDistanceToNow } from 'date-fns';
import { Trash2, User } from 'lucide-react';
import { Button } from '@/components/ui/button';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export interface Comment {
  id: string;
  authorId: string;
  author: {
    firstName: string;
    lastName: string;
    imageUrl?: string;
  };
  text: string;
  createdAt: string;
  lineNumber: number;
}

interface InlineCommentProps {
  comment: Comment;
  onDelete?: (commentId: string) => void;
  canDelete?: boolean;
  className?: string;
}

/**
 * InlineComment - Display single comment with author info and timestamp
 *
 * @example
 * <InlineComment
 *   comment={comment}
 *   onDelete={handleDelete}
 *   canDelete={isOwnComment}
 * />
 */
export function InlineComment({
  comment,
  onDelete,
  canDelete = false,
  className = '',
}: InlineCommentProps) {
  const authorName = `${comment.author.firstName} ${comment.author.lastName}`;
  const timeAgo = formatDistanceToNow(new Date(comment.createdAt), { addSuffix: true });

  return (
    <div className={`flex gap-3 p-3 bg-slate-50 dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-700 ${className}`}>
      {/* Avatar */}
      <div className="flex-shrink-0">
        {comment.author.imageUrl ? (
          <img
            src={comment.author.imageUrl}
            alt={authorName}
            className="w-8 h-8 rounded-full"
          />
        ) : (
          <div className="w-8 h-8 rounded-full bg-blue-100 dark:bg-blue-900 flex items-center justify-center">
            <User className="w-4 h-4 text-blue-600 dark:text-blue-400" />
          </div>
        )}
      </div>

      {/* Comment Content */}
      <div className="flex-1 min-w-0">
        {/* Header: Author + Time */}
        <div className="flex items-center gap-2 mb-1">
          <span className="text-sm font-semibold text-slate-900 dark:text-slate-100">
            {authorName}
          </span>
          <span className="text-xs text-slate-500 dark:text-slate-400">
            {timeAgo}
          </span>
        </div>

        {/* Comment Text (Markdown) */}
        <div className="prose prose-sm dark:prose-invert max-w-none">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>
            {comment.text}
          </ReactMarkdown>
        </div>
      </div>

      {/* Delete Button */}
      {canDelete && onDelete && (
        <div className="flex-shrink-0">
          <Button
            onClick={() => onDelete(comment.id)}
            size="sm"
            variant="ghost"
            className="h-8 w-8 p-0 text-slate-400 hover:text-red-600 dark:hover:text-red-400"
          >
            <Trash2 className="w-4 h-4" />
          </Button>
        </div>
      )}
    </div>
  );
}
