/**
 * InlineCommentForm - Simple inline form for adding comments on diff lines
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { logger } from '@/lib/logger';

interface InlineCommentFormProps {
  filePath: string;
  lineNumber: number;
  onSubmit: (content: string) => Promise<void>;
  onClose: () => void;
}

export function InlineCommentForm({ filePath, lineNumber, onSubmit, onClose }: InlineCommentFormProps) {
  const [content, setContent] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSubmit = useCallback(async () => {
    if (!content.trim() || isSubmitting) return;

    setIsSubmitting(true);
    try {
      await onSubmit(content.trim());
      setContent('');
      onClose();
    } catch (error) {
      logger.error('Failed to add comment:', error);
    } finally {
      setIsSubmitting(false);
    }
  }, [content, isSubmitting, onSubmit, onClose]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onClose();
    } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      handleSubmit();
    }
  }, [onClose, handleSubmit]);

  return (
    <div className="absolute left-0 right-0 top-full z-50 mt-1 mx-2 bg-background rounded-sm shadow-md border border-border p-3">
      <div className="text-xs text-muted-foreground mb-2 flex items-center gap-1">
        <span className="material-symbols-outlined text-[14px]">add_comment</span>
        <span>Comment on line {lineNumber}</span>
        <span className="text-muted-foreground">•</span>
        <span className="truncate max-w-[200px]" title={filePath}>{filePath.split('/').pop()}</span>
      </div>
      <textarea
        ref={textareaRef}
        value={content}
        onChange={(e) => setContent(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Add a comment... (Cmd+Enter to submit)"
        className="w-full px-3 py-2 text-sm bg-muted/30 border border-border rounded-sm resize-none focus:outline-none focus:ring-1 focus:ring-ring"
        rows={2}
        disabled={isSubmitting}
      />
      <div className="flex items-center justify-end gap-2 mt-2">
        <button
          onClick={onClose}
          className="h-7 px-2.5 text-xs font-medium text-muted-foreground border border-border rounded-sm hover:bg-muted/50 transition-colors"
          disabled={isSubmitting}
        >
          Cancel
        </button>
        <button
          onClick={handleSubmit}
          disabled={!content.trim() || isSubmitting}
          className="h-7 px-2.5 text-xs font-medium border border-border rounded-sm text-foreground hover:bg-muted/50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1"
        >
          {isSubmitting && (
            <span className="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
          )}
          Comment
        </button>
      </div>
    </div>
  );
}
