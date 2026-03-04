import { Settings } from 'lucide-react';
import { BaseEntry } from './BaseEntry';
import { RawLogText } from './RawLogText';

interface SystemMessageProps {
  content: string;
  timestamp?: string | null;
}

/**
 * Display system message (configuration, setup, internal notes).
 * System messages appear with muted background.
 */
export function SystemMessage({
  content,
  timestamp,
}: SystemMessageProps) {
  return (
    <BaseEntry variant="system" timestamp={timestamp}>
      <div className="flex items-start gap-3">
        <Settings className="w-5 h-5 text-muted-foreground flex-shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <div className="text-sm text-muted-foreground">
            <RawLogText text={content} />
          </div>
        </div>
      </div>
    </BaseEntry>
  );
}
