import { AlertCircle } from 'lucide-react';
import type { ErrorEntry } from '@/types/timeline-log';
import { formatTimestamp } from '@/utils/formatters';

interface ErrorCardProps {
  error: ErrorEntry;
}

/**
 * Error card for timeline.
 * Displays errors and failures with prominent styling.
 */
export function ErrorCard({ error }: ErrorCardProps) {
  return (
    <div className="relative pl-12">
      {/* Timeline dot */}
      <div
        className="absolute left-[1.875rem] top-3 w-3 h-3 rounded-full border-2 border-background bg-destructive"
        aria-hidden="true"
      />

      {/* Card */}
      <div className="border border-destructive/50 rounded-lg overflow-hidden bg-destructive/5">
        <div className="px-4 py-3">
          {/* Header */}
          <div className="flex items-center gap-2 mb-2">
            <AlertCircle className="w-4 h-4 text-destructive" />
            <span className="text-sm font-medium text-destructive">Error</span>
            {error.tool && (
              <span className="text-xs text-muted-foreground">in {error.tool}</span>
            )}
            <span className="text-xs text-muted-foreground ml-auto">
              {formatTimestamp(error.timestamp)}
            </span>
          </div>

          {/* Error message - red text for visibility */}
          <div className="text-sm text-destructive font-medium bg-destructive/10 border border-destructive/20 rounded px-3 py-2 whitespace-pre-wrap break-words">
            {error.error}
          </div>
        </div>
      </div>
    </div>
  );
}
