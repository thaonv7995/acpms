/**
 * Vibe Kanban-style aggregated file changes.
 * Accordion-style: path + stats, click to open diff.
 */
import { ChevronDown, Pencil } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface AggregatedFileChange {
  path: string;
  changeLabel: string;
  linesAdded: number;
  linesRemoved: number;
  diffId: string;
}

interface ChatAggregatedFileChangesProps {
  file: AggregatedFileChange;
  onViewDiff?: (diffId: string, filePath?: string) => void;
  className?: string;
}

export function ChatAggregatedFileChanges({
  file,
  onViewDiff,
  className,
}: ChatAggregatedFileChangesProps) {
  const canOpen = Boolean(onViewDiff);
  const hasStats = file.linesAdded > 0 || file.linesRemoved > 0;

  return (
    <div
      className={cn(
        'flex items-center gap-2 px-2 py-1.5 rounded-sm border border-border/50 bg-muted/10',
        canOpen && 'cursor-pointer text-emerald-600 hover:text-emerald-500 hover:bg-muted/20 transition-colors',
        !canOpen && 'text-muted-foreground',
        className
      )}
      role={canOpen ? 'button' : undefined}
      tabIndex={canOpen ? 0 : undefined}
      onClick={canOpen ? () => onViewDiff?.(file.diffId, file.path) : undefined}
      onKeyDown={
        canOpen
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onViewDiff?.(file.diffId, file.path);
              }
            }
          : undefined
      }
      title={file.path}
    >
      <Pencil className="h-4 w-4 shrink-0" />
      <span className={cn("min-w-0 flex-1 text-sm truncate", canOpen ? "text-inherit" : "text-foreground")}>
        {file.path}
      </span>
      {hasStats && (
        <div className="flex items-center gap-2 text-xs font-medium tabular-nums shrink-0">
          <span className="text-emerald-500">+{file.linesAdded}</span>
          <span className="text-destructive">-{file.linesRemoved}</span>
        </div>
      )}
      {canOpen && <ChevronDown className="h-4 w-4 shrink-0 text-muted-foreground -rotate-90" />}
    </div>
  );
}
