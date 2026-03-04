import { useState } from 'react';
import { Brain, ChevronDown, ChevronRight } from 'lucide-react';
import { BaseEntry } from './BaseEntry';
import { RawLogText } from './RawLogText';

interface ThinkingEntryProps {
  content: string;
  timestamp?: string | null;
}

/**
 * Display thinking/reasoning entry with expandable content.
 * Thinking entries appear collapsed by default.
 */
export function ThinkingEntry({ content, timestamp }: ThinkingEntryProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <BaseEntry variant="default" timestamp={timestamp}>
      <div
        className="flex items-center gap-3 cursor-pointer group"
        onClick={() => setExpanded(!expanded)}
      >
        <button className="p-0.5 rounded hover:bg-accent transition-colors">
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="w-4 h-4 text-muted-foreground" />
          )}
        </button>
        <Brain className="w-5 h-5 text-cyan-500 flex-shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-cyan-500">Thinking...</div>
        </div>
      </div>

      {expanded && (
        <div className="mt-3 pl-12 border-l-2 border-cyan-500/30 py-2">
          <div className="text-sm text-muted-foreground">
            <RawLogText text={content} />
          </div>
        </div>
      )}
    </BaseEntry>
  );
}
