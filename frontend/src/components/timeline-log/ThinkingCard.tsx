import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, ChevronDown, Brain } from 'lucide-react';
import type { ThinkingEntry } from '@/types/timeline-log';
import { formatTimestamp } from '@/utils/formatters';
import { cn } from '@/lib/utils';

interface ThinkingCardProps {
  thinking: ThinkingEntry;
}

/**
 * Thinking/reasoning card for timeline.
 * Displays agent's internal reasoning with collapsible content.
 */
export function ThinkingCard({ thinking }: ThinkingCardProps) {
  const [expanded, setExpanded] = useState(false);

  // Truncate content for preview (first 100 chars)
  const preview = thinking.content.slice(0, 100);
  const hasMore = thinking.content.length > 100;

  return (
    <div className="relative pl-12">
      {/* Timeline dot */}
      <div
        className="absolute left-[1.875rem] top-3 w-3 h-3 rounded-full border-2 border-background bg-purple-400"
        aria-hidden="true"
      />

      {/* Card */}
      <div
        className={cn(
          'border rounded-lg overflow-hidden transition-colors bg-card',
          expanded ? 'border-purple-400/50' : 'border-border hover:border-purple-400/30'
        )}
      >
        {/* Header */}
        <button
          onClick={() => hasMore && setExpanded(!expanded)}
          className={cn(
            'w-full px-4 py-3 flex items-center gap-3 transition-colors',
            hasMore && 'cursor-pointer hover:bg-purple-400/5'
          )}
        >
          {/* Expand icon */}
          {hasMore && (
            <div className="flex-shrink-0">
              {expanded ? (
                <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />
              ) : (
                <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" />
              )}
            </div>
          )}

          {/* Brain icon */}
          <Brain className="w-4 h-4 text-purple-400 flex-shrink-0" />

          {/* Content preview */}
          <div className="flex-1 text-left min-w-0">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-sm font-medium text-purple-400">Thinking</span>
              <span className="text-xs text-muted-foreground">
                {formatTimestamp(thinking.timestamp)}
              </span>
            </div>
            <div className="text-xs text-muted-foreground italic line-clamp-1">
              {preview}
              {hasMore && '...'}
            </div>
          </div>
        </button>

        {/* Expanded content */}
        <AnimatePresence>
          {expanded && hasMore && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="overflow-hidden"
            >
              <div className="border-t border-purple-400/20 bg-purple-400/5 px-4 py-3">
                <div className="text-sm text-foreground whitespace-pre-wrap break-words max-h-64 overflow-y-auto">
                  {thinking.content}
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
