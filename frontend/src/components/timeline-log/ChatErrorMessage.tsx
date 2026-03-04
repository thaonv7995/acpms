/**
 * Vibe Kanban-style inline error message.
 */
import { useState } from 'react';
import { AlertTriangle } from 'lucide-react';
import { cn } from '@/lib/utils';

interface ChatErrorMessageProps {
  content: string;
  className?: string;
}

export function ChatErrorMessage({ content, className }: ChatErrorMessageProps) {
  const [expanded, setExpanded] = useState(false);
  const isLong = content.length > 120 || content.includes('\n');

  return (
    <div
      className={cn(
        'flex items-start gap-2 text-sm text-destructive cursor-pointer',
        isLong && 'min-w-0',
        className
      )}
      onClick={() => isLong && setExpanded((v) => !v)}
      role={isLong ? 'button' : undefined}
    >
      <AlertTriangle className="shrink-0 pt-0.5 h-4 w-4" />
      <span
        className={cn(
          'min-w-0',
          !expanded && isLong && 'truncate',
          (expanded || !isLong) && 'whitespace-pre-wrap break-all'
        )}
      >
        {content}
      </span>
    </div>
  );
}
