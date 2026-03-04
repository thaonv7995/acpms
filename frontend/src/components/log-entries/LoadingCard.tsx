import { Loader2 } from 'lucide-react';
import { BaseEntry } from './BaseEntry';

interface LoadingCardProps {
  timestamp?: string | null;
}

/**
 * Display loading indicator for pending operations.
 */
export function LoadingCard({ timestamp }: LoadingCardProps) {
  return (
    <BaseEntry variant="default" timestamp={timestamp}>
      <div className="flex items-center gap-3">
        <Loader2 className="w-5 h-5 text-primary flex-shrink-0 animate-spin" />
        <div className="flex-1 min-w-0">
          <div className="text-sm text-muted-foreground">Processing...</div>
        </div>
      </div>
    </BaseEntry>
  );
}
