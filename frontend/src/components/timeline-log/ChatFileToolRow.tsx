/**
 * Vibe Kanban-style file tool row.
 * - file_read: Eye icon + path in backticks
 * - file_edit: Pencil icon + path + +N -M (green/red)
 */
import { Eye, Pencil } from 'lucide-react';
import { cn } from '@/lib/utils';
import { formatLogPathForConversation } from '@/lib/logPathDisplay';

interface ChatFileToolRowProps {
  action: 'file_read' | 'file_edit' | 'file_write';
  path: string;
  linesAdded?: number;
  linesRemoved?: number;
  onViewDiff?: () => void;
  className?: string;
}

export function ChatFileToolRow({
  action,
  path,
  linesAdded = 0,
  linesRemoved = 0,
  onViewDiff,
  className,
}: ChatFileToolRowProps) {
  const isRead = action === 'file_read';
  const isEdit = action === 'file_edit' || action === 'file_write';
  const hasStats = isEdit && (linesAdded > 0 || linesRemoved > 0);
  const isClickable = Boolean(onViewDiff);
  const displayPath = formatLogPathForConversation(path);
  const label = isRead ? `Opened ${displayPath}` : displayPath;

  return (
    <div
      className={cn(
        'flex items-center gap-2 text-sm',
        isRead ? 'text-muted-foreground' : isClickable ? 'text-emerald-600 cursor-pointer hover:text-emerald-500 transition-colors' : 'text-muted-foreground',
        className
      )}
      onClick={onViewDiff}
      role={isClickable ? 'button' : undefined}
    >
      {isRead ? (
        <Eye className="h-4 w-4 shrink-0" />
      ) : (
        <Pencil className="h-4 w-4 shrink-0" />
      )}
      <>
        <span className="min-w-0 flex-1 truncate">{label}</span>
        {hasStats && (
          <span className="shrink-0 text-xs font-medium tabular-nums">
            <span className="text-emerald-500">+{linesAdded}</span>
            <span className="text-destructive"> -{linesRemoved}</span>
          </span>
        )}
      </>
    </div>
  );
}
