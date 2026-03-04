// AddCommentForm - Form for adding a new review comment
import { useState, useRef, useEffect } from 'react';

interface AddCommentFormProps {
  onSubmit: (content: string) => Promise<void>;
  onCancel?: () => void;
  isSubmitting?: boolean;
  placeholder?: string;
  autoFocus?: boolean;
  showCancelButton?: boolean;
  submitLabel?: string;
  compact?: boolean;
}

/**
 * Form component for adding review comments.
 * Supports both inline (compact) and full-size modes.
 *
 * Usage:
 * <AddCommentForm
 *   onSubmit={async (content) => await addComment({ content })}
 *   placeholder="Add a comment..."
 *   autoFocus
 * />
 */
export function AddCommentForm({
  onSubmit,
  onCancel,
  isSubmitting = false,
  placeholder = 'Add a comment...',
  autoFocus = false,
  showCancelButton = true,
  submitLabel = 'Comment',
  compact = false,
}: AddCommentFormProps) {
  const [content, setContent] = useState('');
  const [error, setError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (autoFocus && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [autoFocus]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    const trimmedContent = content.trim();
    if (!trimmedContent) {
      setError('Comment cannot be empty');
      return;
    }

    setError(null);
    try {
      await onSubmit(trimmedContent);
      setContent('');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add comment');
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Submit on Cmd/Ctrl + Enter
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      handleSubmit(e);
    }
    // Cancel on Escape
    if (e.key === 'Escape' && onCancel) {
      e.preventDefault();
      onCancel();
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-2">
      <div className="relative">
        <textarea
          ref={textareaRef}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={isSubmitting}
          rows={compact ? 2 : 3}
          className={`
            w-full px-3 py-2 text-sm
            border border-slate-200 dark:border-slate-700
            rounded-lg
            bg-white dark:bg-slate-800
            text-slate-900 dark:text-slate-100
            placeholder-slate-400 dark:placeholder-slate-500
            focus:ring-2 focus:ring-primary focus:border-transparent
            disabled:opacity-50 disabled:cursor-not-allowed
            resize-none
            ${error ? 'border-red-500 focus:ring-red-500' : ''}
          `}
        />
        {/* Character hint */}
        <span className="absolute bottom-2 right-2 text-[10px] text-slate-400 dark:text-slate-500">
          {content.length > 0 && `${content.length} chars`}
        </span>
      </div>

      {error && (
        <p className="text-xs text-red-500 dark:text-red-400">{error}</p>
      )}

      <div className="flex items-center justify-between">
        <span className="text-[10px] text-slate-400 dark:text-slate-500">
          Press <kbd className="px-1 py-0.5 bg-slate-100 dark:bg-slate-700 rounded text-[10px]">Cmd</kbd>+
          <kbd className="px-1 py-0.5 bg-slate-100 dark:bg-slate-700 rounded text-[10px]">Enter</kbd> to submit
        </span>

        <div className="flex items-center gap-2">
          {showCancelButton && onCancel && (
            <button
              type="button"
              onClick={onCancel}
              disabled={isSubmitting}
              className="px-3 py-1.5 text-xs font-medium text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
          )}
          <button
            type="submit"
            disabled={isSubmitting || !content.trim()}
            className="px-4 py-1.5 text-xs font-bold text-white bg-primary hover:bg-primary/90 rounded-lg shadow-sm disabled:opacity-50 disabled:cursor-not-allowed transition-all flex items-center gap-1.5"
          >
            {isSubmitting ? (
              <>
                <div className="animate-spin rounded-full h-3 w-3 border-b-2 border-white"></div>
                Sending...
              </>
            ) : (
              <>
                <span className="material-symbols-outlined text-[14px]">send</span>
                {submitLabel}
              </>
            )}
          </button>
        </div>
      </div>
    </form>
  );
}
