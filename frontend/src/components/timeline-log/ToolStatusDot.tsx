/**
 * Vibe Kanban-style tool status indicator.
 */
import { cn } from '@/lib/utils';

export interface ToolStatusLike {
  status: string;
}

interface ToolStatusDotProps {
  status: ToolStatusLike;
  className?: string;
}

export function ToolStatusDot({ status, className }: ToolStatusDotProps) {
  const statusType = status.status;

  const isSuccess = statusType === 'success';
  const isError =
    statusType === 'failed' ||
    statusType === 'denied' ||
    statusType === 'timed_out';
  const isPending =
    statusType === 'created' || statusType === 'pending_approval';

  return (
    <span className={cn('inline-flex relative', className)}>
      <span
        className={cn(
          'size-1.5 rounded-full',
          isSuccess && 'bg-emerald-500',
          isError && 'bg-destructive',
          isPending && 'bg-muted-foreground'
        )}
      />
      {isPending && (
        <span className="absolute inset-0 size-1.5 rounded-full bg-muted-foreground animate-ping opacity-75" />
      )}
    </span>
  );
}
