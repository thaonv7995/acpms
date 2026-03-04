import { useState } from 'react';
import { MessageSquarePlus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { CommentThread } from './CommentThread';
import { CommentInput } from './CommentInput';
import { useReview } from '@/contexts/ReviewContext';

interface DiffWithCommentsExampleProps {
  fileId: string;
  filePath: string;
  diffContent: string;
}

/**
 * DiffWithComments - Example integration of comments with diff viewer
 * This shows how to integrate CommentThread and CommentInput with a diff display
 *
 * USAGE: This is a reference implementation. Integrate this pattern into your
 * existing DiffsPanel component by:
 *
 * 1. Wrap DiffsPanel with ReviewProvider
 * 2. Add comment button on each diff line
 * 3. Show CommentThread for lines with comments
 * 4. Show CommentInput when adding new comment
 * 5. Add ReviewActionBar at bottom
 */
export function DiffWithCommentsExample({
  fileId,
  filePath,
  diffContent,
}: DiffWithCommentsExampleProps) {
  const { comments, addComment, deleteComment } = useReview();
  const [activeCommentLine, setActiveCommentLine] = useState<number | null>(null);

  const currentUserId = 'current-user-id'; // Get from auth context
  const fileComments = comments[fileId] || {};

  const handleAddComment = (lineNumber: number) => {
    setActiveCommentLine(lineNumber);
  };

  const handleSubmitComment = (lineNumber: number, text: string) => {
    addComment(fileId, lineNumber, text);
    setActiveCommentLine(null);
  };

  const handleCancelComment = () => {
    setActiveCommentLine(null);
  };

  // Mock diff lines (replace with your actual diff parser)
  const diffLines = diffContent.split('\n').map((line, index) => ({
    number: index + 1,
    content: line,
    type: line.startsWith('+') ? 'added' : line.startsWith('-') ? 'removed' : 'unchanged',
  }));

  return (
    <div className="bg-white dark:bg-slate-900">
      {/* File Header */}
      <div className="px-4 py-2 bg-slate-100 dark:bg-slate-800 border-b border-slate-200 dark:border-slate-700">
        <h3 className="text-sm font-mono text-slate-700 dark:text-slate-300">{filePath}</h3>
      </div>

      {/* Diff Content with Comments */}
      <div className="font-mono text-xs">
        {diffLines.map((line) => {
          const lineComments = fileComments[line.number] || [];
          const hasComments = lineComments.length > 0;
          const isAddingComment = activeCommentLine === line.number;

          return (
            <div key={line.number} className="group">
              {/* Diff Line */}
              <div
                className={`flex items-start hover:bg-slate-50 dark:hover:bg-slate-800/50 ${
                  hasComments ? 'bg-yellow-50 dark:bg-yellow-900/10' : ''
                }`}
              >
                {/* Line Number */}
                <div className="w-12 px-2 py-1 text-right text-slate-400 dark:text-slate-600 select-none shrink-0">
                  {line.number}
                </div>

                {/* Line Content */}
                <div
                  className={`flex-1 px-2 py-1 ${
                    line.type === 'added'
                      ? 'bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-300'
                      : line.type === 'removed'
                      ? 'bg-red-50 dark:bg-red-900/20 text-red-800 dark:text-red-300'
                      : 'text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {line.content}
                </div>

                {/* Add Comment Button */}
                <div className="px-2 py-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
                  <Button
                    onClick={() => handleAddComment(line.number)}
                    size="sm"
                    variant="ghost"
                    className="h-6 w-6 p-0"
                    title="Add comment"
                  >
                    <MessageSquarePlus className="w-3 h-3" />
                  </Button>
                </div>
              </div>

              {/* Comment Thread */}
              {hasComments && (
                <div className="px-14 py-2 bg-slate-50 dark:bg-slate-900/50">
                  <CommentThread
                    comments={lineComments}
                    lineNumber={line.number}
                    onDeleteComment={(commentId) =>
                      deleteComment(fileId, line.number, commentId)
                    }
                    currentUserId={currentUserId}
                    isCollapsible={false}
                  />
                </div>
              )}

              {/* Comment Input */}
              {isAddingComment && (
                <div className="px-14 py-2 bg-slate-50 dark:bg-slate-900/50">
                  <CommentInput
                    lineNumber={line.number}
                    lineContent={line.content}
                    onSubmit={(text) => handleSubmitComment(line.number, text)}
                    onCancel={handleCancelComment}
                  />
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/**
 * Integration Example:
 *
 * ```tsx
 * import { ReviewProvider } from '@/contexts/ReviewContext';
 * import { DiffWithCommentsExample } from '@/components/code-review/DiffWithComments.example';
 * import { ReviewActionBar } from '@/components/code-review/ReviewActionBar';
 *
 * function DiffsPanel({ attemptId, files }) {
 *   return (
 *     <ReviewProvider attemptId={attemptId} currentUserId={currentUser.id}>
 *       <div className="h-full flex flex-col">
 *         <div className="flex-1 overflow-auto">
 *           {files.map((file) => (
 *             <DiffWithCommentsExample
 *               key={file.id}
 *               fileId={file.id}
 *               filePath={file.path}
 *               diffContent={file.diff}
 *             />
 *           ))}
 *         </div>
 *         <ReviewActionBar />
 *       </div>
 *     </ReviewProvider>
 *   );
 * }
 * ```
 */
