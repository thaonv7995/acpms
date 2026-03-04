import { Bot } from 'lucide-react';
import { BaseEntry } from './BaseEntry';
import { RawLogText } from './RawLogText';

interface AssistantMessageProps {
  content: string;
  timestamp?: string | null;
}

/**
 * Display assistant message response.
 * Assistant messages appear with card background.
 */
export function AssistantMessage({
  content,
  timestamp,
}: AssistantMessageProps) {
  return (
    <BaseEntry variant="assistant" timestamp={timestamp}>
      <div className="flex items-start gap-3">
        <Bot className="w-5 h-5 text-primary flex-shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <div className="text-sm text-foreground prose prose-sm dark:prose-invert max-w-none">
            <RawLogText text={content} />
          </div>
        </div>
      </div>
    </BaseEntry>
  );
}
