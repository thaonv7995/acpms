/**
 * Vibe Kanban-style inline tool summary.
 * Icon + summary text + optional status dot.
 */
import { forwardRef } from 'react';
import {
  Eye,
  FileText,
  Globe,
  Hammer,
  ListChecks,
  Search,
  Terminal,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { ToolStatusDot, type ToolStatusLike } from './ToolStatusDot';

interface ChatToolSummaryProps {
  summary: string;
  className?: string;
  expanded?: boolean;
  onToggle?: () => void;
  status?: ToolStatusLike;
  onViewContent?: () => void;
  toolName?: string;
  isTruncated?: boolean;
  actionType?: string;
}

function getIcon(actionType?: string, toolName?: string) {
  if (toolName === 'Bash') return Terminal;
  switch (actionType) {
    case 'file_read':
      return Eye;
    case 'file_edit':
    case 'file_write':
      return FileText;
    case 'search':
      return Search;
    case 'web_fetch':
      return Globe;
    case 'command_run':
      return Terminal;
    case 'todo_management':
      return ListChecks;
    default:
      return Hammer;
  }
}

export const ChatToolSummary = forwardRef<HTMLSpanElement, ChatToolSummaryProps>(
  function ChatToolSummary(
    {
      summary,
      className,
      expanded,
      onToggle,
      status,
      onViewContent,
      toolName,
      isTruncated,
      actionType,
    },
    ref
  ) {
    const canExpand = isTruncated && onToggle;
    const isClickable = Boolean(onViewContent || canExpand);

    const handleClick = () => {
      if (onViewContent) {
        onViewContent();
      } else if (canExpand) {
        onToggle?.();
      }
    };

    const Icon = getIcon(actionType, toolName);

    return (
      <div
        className={cn(
          'flex items-center gap-2 text-sm text-muted-foreground',
          isClickable && 'cursor-pointer',
          className
        )}
        onClick={isClickable ? handleClick : undefined}
        role={isClickable ? 'button' : undefined}
      >
        <span className="relative shrink-0 pt-0.5">
          <Icon className="h-4 w-4" />
          {status && (
            <ToolStatusDot
              status={status}
              className="absolute -bottom-0.5 -left-0.5"
            />
          )}
        </span>
        <span
          ref={ref}
          className={cn(
            !expanded && 'truncate',
            expanded && 'whitespace-pre-wrap break-all'
          )}
        >
          {summary}
        </span>
      </div>
    );
  }
);
