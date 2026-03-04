import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';
import { formatTimestamp } from '@/utils/formatters';

type TimelineTone = 'neutral' | 'info' | 'success' | 'warning' | 'destructive';

const toneStyles: Record<
  TimelineTone,
  {
    marker: string;
    card: string;
    title: string;
    badge: string;
  }
> = {
  neutral: {
    marker: 'bg-background text-foreground border-border',
    card: 'border-l-4 border-neutral/60',
    title: 'text-foreground',
    badge: 'bg-muted text-muted-foreground',
  },
  info: {
    marker: 'bg-info/15 text-info border-info/30',
    card: 'border-l-4 border-info/40',
    title: 'text-info',
    badge: 'bg-info/15 text-info',
  },
  success: {
    marker: 'bg-success/15 text-success border-success/30',
    card: 'border-l-4 border-success/40',
    title: 'text-success',
    badge: 'bg-success/15 text-success',
  },
  warning: {
    marker: 'bg-warning/15 text-warning border-warning/30',
    card: 'border-l-4 border-warning/40',
    title: 'text-warning',
    badge: 'bg-warning/15 text-warning',
  },
  destructive: {
    marker: 'bg-destructive/15 text-destructive border-destructive/30',
    card: 'border-l-4 border-destructive/40',
    title: 'text-destructive',
    badge: 'bg-destructive/15 text-destructive',
  },
};

function formatShortTime(timestamp?: string): string | null {
  if (!timestamp) return null;
  try {
    const date = new Date(timestamp);
    if (Number.isNaN(date.getTime())) return null;
    return date.toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return null;
  }
}

interface TimelineRowProps {
  icon: ReactNode;
  title: ReactNode;
  timestamp?: string;
  tone?: TimelineTone;
  badge?: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  headerClassName?: string;
  bodyClassName?: string;
}

/**
 * Shared layout wrapper for timeline entries.
 * Provides left rail (icon + time), consistent card shell, and header row.
 */
export function TimelineRow({
  icon,
  title,
  timestamp,
  tone = 'neutral',
  badge,
  meta,
  actions,
  children,
  className,
  headerClassName,
  bodyClassName,
}: TimelineRowProps) {
  const styles = toneStyles[tone];
  const shortTime = formatShortTime(timestamp);

  return (
    <div className={cn('group grid grid-cols-[4.5rem,1fr] gap-3 px-4 py-2', className)}>
      <div className="flex flex-col items-center pt-1">
        <div
          className={cn(
            'flex h-7 w-7 items-center justify-center rounded-full border shadow-sm transition-colors',
            styles.marker
          )}
        >
          {icon}
        </div>
        {shortTime && (
          <span className="mt-2 text-[10px] text-muted-foreground tracking-wide">
            {shortTime}
          </span>
        )}
      </div>

      <div
        className={cn(
          'min-w-0 rounded-xl border bg-card/80 shadow-sm backdrop-blur-[1px] transition-colors group-hover:border-border',
          styles.card
        )}
      >
        <div
          className={cn(
            'flex items-center gap-2 px-4 py-2 border-b border-border/50 bg-muted/30',
            headerClassName
          )}
        >
          <span className={cn('text-[11px] font-semibold uppercase tracking-wide', styles.title)}>
            {title}
          </span>
          {badge}
          {meta}
          <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
            {timestamp && <span>{formatTimestamp(timestamp)}</span>}
            {actions}
          </div>
        </div>
        <div className={cn('px-4 py-3', bodyClassName)}>{children}</div>
      </div>
    </div>
  );
}
