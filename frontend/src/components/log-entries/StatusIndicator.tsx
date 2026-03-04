import { CheckCircle, XCircle, Clock, AlertCircle, Circle } from 'lucide-react';
import { cn } from '@/lib/utils';

interface StatusIndicatorProps {
  status: 'created' | 'success' | 'failed' | 'denied' | 'pending_approval' | 'timed_out';
  size?: 'sm' | 'md';
}

/**
 * Visual indicator for tool execution status.
 */
export function StatusIndicator({ status, size = 'sm' }: StatusIndicatorProps) {
  const iconSize = size === 'sm' ? 'w-4 h-4' : 'w-5 h-5';

  switch (status) {
    case 'success':
      return (
        <CheckCircle
          className={cn(iconSize, 'text-green-500 flex-shrink-0')}
          aria-label="Success"
        />
      );
    case 'failed':
      return (
        <XCircle
          className={cn(iconSize, 'text-destructive flex-shrink-0')}
          aria-label="Failed"
        />
      );
    case 'denied':
      return (
        <AlertCircle
          className={cn(iconSize, 'text-orange-500 flex-shrink-0')}
          aria-label="Denied"
        />
      );
    case 'pending_approval':
      return (
        <Clock
          className={cn(iconSize, 'text-yellow-500 flex-shrink-0 animate-pulse')}
          aria-label="Pending Approval"
        />
      );
    case 'timed_out':
      return (
        <XCircle
          className={cn(iconSize, 'text-orange-500 flex-shrink-0')}
          aria-label="Timed Out"
        />
      );
    case 'created':
    default:
      return (
        <Circle
          className={cn(iconSize, 'text-muted-foreground flex-shrink-0')}
          aria-label="Created"
        />
      );
  }
}
