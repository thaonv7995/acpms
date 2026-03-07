/**
 * Vibe Kanban-style collapsible thinking group.
 * When collapsed: "Thinking" + icon. When hovered: chevron. When expanded: list of entries.
 */
import { useState } from 'react';
import { Brain, ChevronRight } from 'lucide-react';
import { cn } from '@/lib/utils';
import { timelineT } from './timeline-i18n';

export interface ThinkingEntry {
  content: string;
  expansionKey: string;
}

interface ChatCollapsedThinkingProps {
  entries: ThinkingEntry[];
  expanded: boolean;
  onToggle: () => void;
  renderMarkdown: (content: string) => React.ReactNode;
  className?: string;
}

export function ChatCollapsedThinking({
  entries,
  expanded,
  onToggle,
  renderMarkdown,
  className,
}: ChatCollapsedThinkingProps) {
  const [isHovered, setIsHovered] = useState(false);

  if (entries.length === 0) return null;

  return (
    <div className={cn('flex flex-col', className)}>
      <div
        className="flex items-center gap-2 text-xs text-muted-foreground/75 cursor-pointer"
        onClick={onToggle}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        role="button"
        aria-expanded={expanded}
      >
        <span className="shrink-0 pt-0.5">
          {isHovered ? (
            <ChevronRight
              className={cn(
                'h-4 w-4 transition-transform duration-150',
                expanded && 'rotate-90'
              )}
            />
          ) : (
            <Brain className="h-4 w-4" />
          )}
        </span>
        <span className="truncate">{timelineT.thinking}</span>
      </div>

      {expanded && (
        <div className="ml-6 pt-2 flex flex-col gap-2">
          {entries.map((entry) => (
            <div key={entry.expansionKey} className="text-xs text-muted-foreground/75 pl-4">
              {renderMarkdown(entry.content)}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
