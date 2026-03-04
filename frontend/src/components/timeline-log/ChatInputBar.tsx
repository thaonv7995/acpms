import { useState, useRef, useEffect, type KeyboardEvent } from 'react';
import { Send, Loader2, WifiOff } from 'lucide-react';
import { cn } from '@/lib/utils';
import { logger } from '@/lib/logger';

interface ChatInputBarProps {
  onSend: (message: string) => Promise<void>;
  disabled?: boolean;
  placeholder?: string;
  maxLength?: number;
}

const MIN_HEIGHT = 48;
const MAX_HEIGHT = 120;

/**
 * Interactive chat input bar with auto-expanding textarea.
 * Supports Enter to send, Shift+Enter for newline.
 */
export function ChatInputBar({
  onSend,
  disabled = false,
  placeholder = 'Send a message...',
  maxLength = 2000,
}: ChatInputBarProps) {
  const [message, setMessage] = useState('');
  const [isSending, setIsSending] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      const scrollHeight = textareaRef.current.scrollHeight;
      textareaRef.current.style.height = `${Math.min(
        Math.max(scrollHeight, MIN_HEIGHT),
        MAX_HEIGHT
      )}px`;
    }
  }, [message]);

  const handleSend = async () => {
    const trimmed = message.trim();
    if (!trimmed || isSending || disabled) return;

    setIsSending(true);
    try {
      await onSend(trimmed);
      setMessage('');
      if (textareaRef.current) {
        textareaRef.current.style.height = `${MIN_HEIGHT}px`;
      }
    } catch (error) {
      logger.error('Failed to send message:', error);
    } finally {
      setIsSending(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter to send, Shift+Enter for newline
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const charCount = message.length;
  const isOverLimit = charCount > maxLength;
  const canSend = message.trim() && !isSending && !disabled && !isOverLimit;

  return (
    <div className="p-4 bg-background">
      <div className={cn(
        "flex flex-col rounded-xl border bg-muted/10 transition-all",
        disabled ? "opacity-70 border-border/50" : "border-border shadow-[0_2px_8px_rgba(0,0,0,0.04)] focus-within:border-border/80 focus-within:shadow-[0_2px_12px_rgba(0,0,0,0.08)] bg-muted/20",
        isOverLimit && "border-destructive focus-within:border-destructive"
      )}>
        <textarea
          ref={textareaRef}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={disabled || isSending}
          className={cn(
            "w-full resize-none bg-transparent px-4 py-3 text-[14px] leading-relaxed text-foreground placeholder:text-muted-foreground focus:outline-none",
            disabled && "cursor-not-allowed"
          )}
          style={{
            minHeight: `${MIN_HEIGHT}px`,
            maxHeight: `${MAX_HEIGHT}px`,
          }}
          aria-label="Chat message input"
        />

        <div className="flex items-center justify-between px-3 pb-2 pt-1 border-t border-transparent">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            {disabled && (
              <span className="flex items-center gap-1.5 text-amber-500/90 font-medium">
                <WifiOff className="w-3.5 h-3.5" />
                Agent is not running
              </span>
            )}
            {!disabled && (
              <span className="hidden sm:inline-block">
                <kbd className="font-sans px-1.5 border-b-2 py-0.5 rounded-md bg-background border border-border shadow-sm text-[11px]">Enter</kbd> to send,{' '}
                <kbd className="font-sans px-1.5 border-b-2 py-0.5 rounded-md bg-background border border-border shadow-sm text-[11px]">Shift+Enter</kbd> for newline
              </span>
            )}
            {charCount > 0 && (
              <span className={cn("font-mono ml-2", isOverLimit && "text-destructive font-medium")}>
                {charCount}/{maxLength}
              </span>
            )}
          </div>

          <button
            onClick={handleSend}
            disabled={!canSend}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-lg transition-all flex-shrink-0",
              canSend
                ? "bg-foreground text-background hover:bg-foreground/90 shadow-sm"
                : "bg-muted text-muted-foreground cursor-not-allowed"
            )}
            aria-label="Send message"
          >
            {isSending ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-3.5 h-3.5" />}
          </button>
        </div>
      </div>
    </div>
  );
}
