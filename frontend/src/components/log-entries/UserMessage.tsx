import { User, AlertCircle } from 'lucide-react';
import { BaseEntry } from './BaseEntry';
import { RawLogText } from './RawLogText';

interface UserMessageProps {
  content: string;
  deniedTool?: string;
  timestamp?: string | null;
}

/**
 * Display user message or feedback with optional denied tool indicator.
 * User messages appear with light gray background and dashed border.
 */
export function UserMessage({
  content,
  deniedTool,
  timestamp,
}: UserMessageProps) {
  return (
    <BaseEntry variant="user" timestamp={timestamp}>
      <div className="flex items-start gap-3">
        <User className="w-5 h-5 text-muted-foreground flex-shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          {/* Denied tool indicator */}
          {deniedTool && (
            <div className="flex items-center gap-2 text-xs text-destructive mb-2 p-2 bg-destructive/10 rounded">
              <AlertCircle className="w-3 h-3 flex-shrink-0" />
              <span>Tool denied: {deniedTool}</span>
            </div>
          )}

          {/* Message text with URL linkification */}
          <div className="text-sm text-foreground prose prose-sm dark:prose-invert max-w-none">
            <RawLogText text={content} />
          </div>
        </div>
      </div>
    </BaseEntry>
  );
}
