/**
 * ChatInput - Input with @mentions support
 * Allows user to send follow-up messages to the agent
 */

import {
  memo,
  useState,
  useRef,
  useCallback,
  useEffect,
  KeyboardEvent,
  ChangeEvent,
} from 'react';
import { MentionPopover } from './MentionPopover';
import type { MentionItem } from './types';

interface ChatInputProps {
  onSend: (message: string) => void;
  disabled?: boolean;
  placeholder?: string;
  isLoading?: boolean;
  projectId?: string;
  className?: string;
}

export const ChatInput = memo(function ChatInput({
  onSend,
  disabled = false,
  placeholder = 'Continue working on this task... Type @ to search files',
  isLoading = false,
  projectId = '',
  className = '',
}: ChatInputProps) {
  const [value, setValue] = useState('');
  const [showMentionPopover, setShowMentionPopover] = useState(false);
  const [mentionQuery, setMentionQuery] = useState('');
  const [mentionPosition, setMentionPosition] = useState({ top: 0, left: 0 });
  const [mentionStartIndex, setMentionStartIndex] = useState(-1);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    if (inputRef.current) {
      inputRef.current.style.height = 'auto';
      inputRef.current.style.height = `${Math.min(inputRef.current.scrollHeight, 150)}px`;
    }
  }, [value]);

  // Handle input change and detect @ mentions
  const handleChange = useCallback((e: ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value;
    const cursorPos = e.target.selectionStart || 0;
    setValue(newValue);

    // Check for @ mention trigger
    const textBeforeCursor = newValue.slice(0, cursorPos);
    const atIndex = textBeforeCursor.lastIndexOf('@');

    if (atIndex !== -1) {
      const textAfterAt = textBeforeCursor.slice(atIndex + 1);
      // Check if we're in a valid mention context (no spaces since @)
      if (!textAfterAt.includes(' ') && textAfterAt.length <= 50) {
        setMentionQuery(textAfterAt);
        setMentionStartIndex(atIndex);
        setShowMentionPopover(true);

        // Calculate popover position
        if (inputRef.current) {
          setMentionPosition({
            top: -220, // Show above input
            left: Math.min(atIndex * 8, 200), // Approximate character position
          });
        }
        return;
      }
    }

    setShowMentionPopover(false);
  }, []);

  // Handle mention selection
  const handleMentionSelect = useCallback(
    (item: MentionItem) => {
      if (mentionStartIndex === -1) return;

      // Replace @query with @filepath
      const beforeMention = value.slice(0, mentionStartIndex);
      const afterMention = value.slice(
        mentionStartIndex + mentionQuery.length + 1
      );
      const newValue = `${beforeMention}@${item.value} ${afterMention}`;

      setValue(newValue);
      setShowMentionPopover(false);
      setMentionStartIndex(-1);

      // Focus back to input
      inputRef.current?.focus();
    },
    [value, mentionStartIndex, mentionQuery]
  );

  // Handle keyboard events
  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      // Don't handle if mention popover is open (it handles its own keys)
      if (showMentionPopover) {
        if (['ArrowUp', 'ArrowDown', 'Enter', 'Escape'].includes(e.key)) {
          // Let popover handle these
          return;
        }
      }

      // Submit on Enter (without Shift)
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [showMentionPopover, value]
  );

  // Handle send
  const handleSend = useCallback(() => {
    const trimmedValue = value.trim();
    if (!trimmedValue || disabled || isLoading) return;

    onSend(trimmedValue);
    setValue('');
    setShowMentionPopover(false);
  }, [value, disabled, isLoading, onSend]);

  // Close mention popover
  const handleCloseMention = useCallback(() => {
    setShowMentionPopover(false);
  }, []);

  return (
    <div className={`relative ${className}`}>
      {/* Mention popover */}
      <MentionPopover
        isOpen={showMentionPopover}
        searchQuery={mentionQuery}
        position={mentionPosition}
        onSelect={handleMentionSelect}
        onClose={handleCloseMention}
        projectId={projectId}
      />

      {/* Input container */}
      <div className="relative bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-600 rounded-lg focus-within:ring-2 focus-within:ring-primary/20 focus-within:border-primary transition-colors">
        {/* Textarea */}
        <textarea
          ref={inputRef}
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={disabled || isLoading}
          rows={1}
          className="w-full px-4 py-3 pr-32 bg-transparent text-sm text-slate-900 dark:text-white placeholder-slate-400 resize-none focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed"
          style={{ minHeight: '48px', maxHeight: '150px' }}
        />

        {/* Action buttons */}
        <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
          {/* Attach file button */}
          <button
            type="button"
            className="p-1.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 transition-colors disabled:opacity-50"
            title="Attach file"
            disabled={disabled || isLoading}
          >
            <span className="material-symbols-outlined text-[18px]">attach_file</span>
          </button>

          {/* Terminal button */}
          <button
            type="button"
            className="p-1.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 transition-colors disabled:opacity-50"
            title="Run command"
            disabled={disabled || isLoading}
          >
            <span className="material-symbols-outlined text-[18px]">terminal</span>
          </button>

          {/* Send button */}
          <button
            onClick={handleSend}
            disabled={!value.trim() || disabled || isLoading}
            className="px-4 py-1.5 bg-primary hover:bg-primary/90 disabled:bg-slate-300 dark:disabled:bg-slate-700 text-white disabled:text-slate-500 rounded-md text-sm font-medium transition-colors flex items-center gap-1"
          >
            {isLoading ? (
              <>
                <div className="animate-spin rounded-full h-3 w-3 border-b-2 border-white" />
                <span>Sending</span>
              </>
            ) : (
              <>
                <span className="material-symbols-outlined text-[16px]">send</span>
                <span>Send</span>
              </>
            )}
          </button>
        </div>
      </div>

      {/* Help text */}
      <div className="flex items-center gap-4 mt-2 text-xs text-slate-500">
        <span className="flex items-center gap-1">
          <kbd className="px-1.5 py-0.5 bg-slate-200 dark:bg-slate-700 rounded text-[10px] font-mono">
            @
          </kbd>
          mention files
        </span>
        <span className="flex items-center gap-1">
          <kbd className="px-1.5 py-0.5 bg-slate-200 dark:bg-slate-700 rounded text-[10px] font-mono">
            Shift + Enter
          </kbd>
          new line
        </span>
        <span className="flex items-center gap-1">
          <kbd className="px-1.5 py-0.5 bg-slate-200 dark:bg-slate-700 rounded text-[10px] font-mono">
            Enter
          </kbd>
          send
        </span>
      </div>
    </div>
  );
});
