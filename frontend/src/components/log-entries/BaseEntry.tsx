import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';
import { formatTimestamp } from '@/utils/formatters';

interface BaseEntryProps {
  children: ReactNode;
  variant?: 'default' | 'user' | 'assistant' | 'system' | 'error' | 'action';
  timestamp?: string | null;
  className?: string;
}

const variantStyles = {
  default: 'bg-card border-border',
  user: 'bg-background border-y border-dashed border-border',
  assistant: 'bg-card border-border',
  system: 'bg-muted/50 border-muted',
  error: 'bg-destructive/10 border-destructive/20',
  action: 'bg-primary/5 border-primary/20',
};

/**
 * Base wrapper for all log entry components.
 * Provides consistent padding, borders, and spacing.
 */
export function BaseEntry({
  children,
  variant = 'default',
  timestamp,
  className,
}: BaseEntryProps) {
  return (
    <div
      className={cn(
        'flex flex-col gap-1 px-4 py-3 border rounded-sm',
        variantStyles[variant],
        className
      )}
    >
      {children}
      {timestamp && (
        <div className="text-xs text-muted-foreground mt-2">
          {formatTimestamp(timestamp)}
        </div>
      )}
    </div>
  );
}
