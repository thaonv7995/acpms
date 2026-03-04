import { AlertCircle } from 'lucide-react';
import { BaseEntry } from './BaseEntry';
import { RawLogText } from './RawLogText';
import type { NormalizedEntryError } from '@/bindings/NormalizedEntryError';

interface ErrorMessageProps {
  content: string;
  errorType?: NormalizedEntryError;
  timestamp?: string | null;
}

/**
 * Display error message with optional error type classification.
 * Error messages appear with red background indicating failure state.
 */
export function ErrorMessage({
  content,
  errorType,
  timestamp,
}: ErrorMessageProps) {
  return (
    <BaseEntry variant="error" timestamp={timestamp}>
      <div className="flex items-start gap-3">
        <AlertCircle className="w-5 h-5 text-destructive flex-shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <div className="font-medium text-sm text-destructive mb-1">
            Error
            {errorType && ` (${errorType})`}
          </div>
          <div className="text-sm text-destructive/80">
            <RawLogText text={content} />
          </div>
        </div>
      </div>
    </BaseEntry>
  );
}
