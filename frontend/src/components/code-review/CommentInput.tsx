import { useState, useRef, useEffect } from 'react';
import { Send, X, Eye } from 'lucide-react';
import { Button } from '@/components/ui/button';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface CommentInputProps {
  onSubmit: (text: string) => void;
  onCancel: () => void;
  lineNumber: number;
  lineContent?: string;
  maxLength?: number;
  placeholder?: string;
  className?: string;
}

/**
 * CommentInput - Input form for adding new comments
 * Features: Markdown preview, character count, keyboard shortcuts
 *
 * @example
 * <CommentInput
 *   lineNumber={42}
 *   onSubmit={handleSubmit}
 *   onCancel={handleCancel}
 * />
 */
export function CommentInput({
  onSubmit,
  onCancel,
  lineNumber,
  lineContent,
  maxLength = 5000,
  placeholder = 'Add a comment... (Markdown supported)',
  className = '',
}: CommentInputProps) {
  const [text, setText] = useState('');
  const [showPreview, setShowPreview] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const charCount = text.length;
  const isOverLimit = charCount > maxLength;
  const canSubmit = text.trim().length > 0 && !isOverLimit;

  // Autofocus on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSubmit = () => {
    if (!canSubmit) return;
    onSubmit(text.trim());
    setText('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Ctrl/Cmd+Enter to submit
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      handleSubmit();
    }
    // Escape to cancel
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  };

  return (
    <div className={`border border-slate-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-800 ${className}`}>
      {/* Line Context (optional) */}
      {lineContent && (
        <div className="px-3 py-2 bg-slate-50 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-700">
          <span className="text-xs text-slate-500 dark:text-slate-400">
            Line {lineNumber}:
          </span>
          <code className="ml-2 text-xs font-mono text-slate-700 dark:text-slate-300">
            {lineContent.slice(0, 80)}
            {lineContent.length > 80 && '...'}
          </code>
        </div>
      )}

      {/* Input/Preview Area */}
      <div className="p-3">
        {showPreview ? (
          // Preview Mode
          <div className="min-h-[80px] prose prose-sm dark:prose-invert max-w-none">
            {text.trim() ? (
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
            ) : (
              <p className="text-slate-400 dark:text-slate-500 italic">
                Nothing to preview
              </p>
            )}
          </div>
        ) : (
          // Edit Mode
          <textarea
            ref={textareaRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            className="w-full min-h-[80px] resize-none bg-transparent border-none outline-none text-sm text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500"
            maxLength={maxLength}
          />
        )}
      </div>

      {/* Footer: Controls + Character Count */}
      <div className="flex items-center justify-between px-3 py-2 bg-slate-50 dark:bg-slate-900 border-t border-slate-200 dark:border-slate-700">
        {/* Left: Preview Toggle */}
        <Button
          onClick={() => setShowPreview(!showPreview)}
          size="sm"
          variant="ghost"
          className="gap-2 text-slate-600 dark:text-slate-400"
        >
          <Eye className="w-4 h-4" />
          {showPreview ? 'Edit' : 'Preview'}
        </Button>

        {/* Right: Character Count + Actions */}
        <div className="flex items-center gap-2">
          <span
            className={`text-xs ${
              isOverLimit
                ? 'text-red-600 dark:text-red-400 font-semibold'
                : 'text-slate-500 dark:text-slate-400'
            }`}
          >
            {charCount}/{maxLength}
          </span>

          <Button onClick={onCancel} size="sm" variant="ghost">
            <X className="w-4 h-4" />
            Cancel
          </Button>

          <Button
            onClick={handleSubmit}
            disabled={!canSubmit}
            size="sm"
            variant="default"
            className="gap-2"
          >
            <Send className="w-4 h-4" />
            Comment
          </Button>
        </div>
      </div>

      {/* Hint */}
      <div className="px-3 pb-2 text-xs text-slate-500 dark:text-slate-400">
        Press <kbd className="px-1 py-0.5 bg-slate-200 dark:bg-slate-700 rounded">Ctrl+Enter</kbd> to submit
      </div>
    </div>
  );
}
