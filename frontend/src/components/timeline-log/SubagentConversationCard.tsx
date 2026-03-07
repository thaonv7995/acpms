import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, ChevronDown, Bot, AlertTriangle } from 'lucide-react';
import type { SubagentEntry } from '@/types/timeline-log';
import { cn } from '@/lib/utils';
import { formatTimestamp } from '@/utils/formatters';
import { TimelineEntryRenderer } from './TimelineEntryRenderer';

interface SubagentConversationCardProps {
  subagent: SubagentEntry;
}

const MAX_DEPTH = 3;

/**
 * Nested subagent display with recursive timeline.
 * Shows subagent task, status, and nested timeline entries.
 */
export function SubagentConversationCard({ subagent }: SubagentConversationCardProps) {
  const [expanded, setExpanded] = useState(false);
  const { thread } = subagent;

  // Calculate indentation based on depth
  const indentClass = `ml-${Math.min(thread.depth, MAX_DEPTH) * 8}`;

  // Show warning if depth exceeds max
  const depthWarning = thread.depth > MAX_DEPTH;

  // Get status color
  const getStatusColor = () => {
    switch (thread.status) {
      case 'running':
        return 'text-warning';
      case 'completed':
        return 'text-success';
      case 'failed':
        return 'text-destructive';
      default:
        return 'text-muted-foreground';
    }
  };

  // Get status label
  const getStatusLabel = () => {
    switch (thread.status) {
      case 'running':
        return 'Running';
      case 'completed':
        return 'Completed';
      case 'failed':
        return 'Failed';
      case 'pending':
        return 'Pending';
      default:
        return 'Unknown';
    }
  };

  const statusColor = getStatusColor();
  const statusLabel = getStatusLabel();

  return (
    <div className={cn('relative pl-12', thread.depth > 0 && indentClass)}>
      {/* Timeline dot */}
      <div
        className={cn(
          'absolute left-[1.875rem] top-3 w-3 h-3 rounded-full border-2 border-background',
          thread.status === 'failed'
            ? 'bg-destructive'
            : thread.status === 'running'
            ? 'bg-warning animate-pulse'
            : thread.status === 'completed'
            ? 'bg-success'
            : 'bg-purple-500'
        )}
        aria-hidden="true"
      />

      {/* Card */}
      <div
        className={cn(
          'border rounded-lg overflow-hidden transition-colors',
          expanded
            ? 'border-purple-500/50 bg-purple-500/5'
            : 'border-border hover:border-purple-500/30 bg-card'
        )}
      >
        {/* Header */}
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full px-4 py-3 flex items-center gap-3 hover:bg-purple-500/10 transition-colors"
        >
          {/* Expand icon */}
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-muted-foreground flex-shrink-0" />
          ) : (
            <ChevronRight className="w-4 h-4 text-muted-foreground flex-shrink-0" />
          )}

          {/* Robot icon with depth badge */}
          <div className="relative flex-shrink-0">
            <Bot className="w-4 h-4 text-purple-500" />
            {thread.depth > 0 && (
              <div className="absolute -top-2 -right-2 bg-purple-500 text-white text-xs font-bold rounded-full w-5 h-5 flex items-center justify-center">
                {thread.depth}
              </div>
            )}
          </div>

          {/* Task info */}
          <div className="flex-1 text-left min-w-0">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-sm font-medium text-purple-500">
                {thread.agentName}
              </span>
              <span className={cn('text-xs font-medium', statusColor)}>
                {statusLabel}
              </span>
              <span className="text-xs text-muted-foreground">
                {formatTimestamp(thread.startedAt)}
              </span>
            </div>
            <div className="text-xs text-foreground line-clamp-2">
              {thread.taskDescription}
            </div>
          </div>

          {/* Depth warning */}
          {depthWarning && (
            <AlertTriangle
              className="w-4 h-4 text-warning flex-shrink-0"
              aria-label={`Max nesting depth (${MAX_DEPTH}) exceeded`}
            />
          )}

          {/* Entry count badge */}
          {thread.entries.length > 0 && (
            <div className="flex-shrink-0 px-2 py-1 bg-purple-500/20 text-purple-500 text-xs font-medium rounded">
              {thread.entries.length}
            </div>
          )}
        </button>

        {/* Expanded nested timeline */}
        <AnimatePresence>
          {expanded && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
              className="overflow-hidden"
            >
              <div className="border-t border-purple-500/20 bg-purple-500/5">
                {thread.entries.length > 0 ? (
                  <div className="p-4 space-y-3 max-h-96 overflow-y-auto">
                    {thread.entries.map((entry) => (
                      <div key={entry.id} className="scale-95 origin-top-left">
                        <TimelineEntryRenderer entry={entry} />
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="px-4 py-8 text-center text-sm text-muted-foreground">
                    No entries yet
                  </div>
                )}

                {/* Completion info */}
                {thread.completedAt && (
                  <div className="px-4 py-2 border-t border-purple-500/20 text-xs text-muted-foreground">
                    Completed at {formatTimestamp(thread.completedAt)}
                    {thread.startedAt && (
                      <>
                        {' · Duration: '}
                        {Math.round(
                          (new Date(thread.completedAt).getTime() -
                            new Date(thread.startedAt).getTime()) /
                            1000
                        )}
                        s
                      </>
                    )}
                  </div>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
