import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, ChevronDown, CheckCircle2, XCircle, Clock } from 'lucide-react';
import type { ToolCallEntry } from '@/types/timeline-log';
import { formatShellCommandForDisplay } from '@/lib/commandDisplay';
import { formatLogPathForDisplay } from '@/lib/logPathDisplay';
import { cn } from '@/lib/utils';
import { getActionIcon, getActionLabel } from '@/utils/icon-mapping';
import { formatTimestamp } from '@/utils/formatters';

interface ToolCallTimelineCardProps {
  toolCall: ToolCallEntry;
  onViewDiff?: (diffId: string) => void;
}

/**
 * Enhanced tool call display for timeline.
 * Shows tool icon, name, duration, target path, and expandable details.
 */
export function ToolCallTimelineCard({ toolCall, onViewDiff }: ToolCallTimelineCardProps) {
  const [expanded, setExpanded] = useState(false);
  const Icon = getActionIcon(toolCall.actionType.action);
  const label = getActionLabel(toolCall.actionType.action);

  // Get target path/file
  const target =
    toolCall.actionType.file_path ||
    toolCall.actionType.path ||
    toolCall.actionType.target;
  const displayTarget = target ? formatLogPathForDisplay(target) : null;

  // Determine if expandable (has meaningful details)
  const hasDetails =
    toolCall.actionType.action === 'command_run' ||
    toolCall.actionType.action === 'file_edit' ||
    toolCall.actionType.action === 'search';

  // Get status icon and color
  const getStatusIndicator = () => {
    switch (toolCall.status) {
      case 'success':
        return <CheckCircle2 className="w-3.5 h-3.5 text-success" />;
      case 'failed':
        return <XCircle className="w-3.5 h-3.5 text-destructive" />;
      case 'running':
        return <Clock className="w-3.5 h-3.5 text-warning animate-pulse" />;
      default:
        return null;
    }
  };

  return (
    <div className="relative pl-12">
      {/* Timeline dot */}
      <div
        className={cn(
          'absolute left-[1.875rem] top-3 w-3 h-3 rounded-full border-2 border-background',
          toolCall.status === 'failed'
            ? 'bg-destructive'
            : toolCall.status === 'running'
            ? 'bg-warning animate-pulse'
            : 'bg-success'
        )}
        aria-hidden="true"
      />

      {/* Card */}
      <div
        className={cn(
          'border rounded-lg overflow-hidden transition-colors bg-card',
          expanded ? 'border-primary/50' : 'border-border hover:border-primary/30',
          toolCall.status === 'failed' && 'border-destructive/30'
        )}
      >
        {/* Main content */}
        <div
          className={cn(
            'px-4 py-2.5 flex items-center gap-3',
            hasDetails && 'cursor-pointer hover:bg-muted/30 transition-colors'
          )}
          onClick={() => hasDetails && setExpanded(!expanded)}
        >
          {/* Expand icon (if expandable) */}
          {hasDetails && (
            <div className="flex-shrink-0">
              {expanded ? (
                <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />
              ) : (
                <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" />
              )}
            </div>
          )}

          {/* Tool icon */}
          <Icon className="w-4 h-4 text-muted-foreground flex-shrink-0" />

          {/* Tool info */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-0.5">
              <span className="text-sm font-medium text-foreground">
                {label === 'Tool' && toolCall.toolName ? toolCall.toolName : label}
              </span>
              {toolCall.duration && (
                <span className="text-xs text-muted-foreground">
                  ({toolCall.duration}ms)
                </span>
              )}
              {/* Diff stats inline */}
              {toolCall.diffStats && (
                <div className="flex items-center gap-1.5 text-xs ml-1">
                  <span className="text-success font-medium">
                    +{toolCall.diffStats.additions}
                  </span>
                  <span className="text-destructive font-medium">
                    -{toolCall.diffStats.deletions}
                  </span>
                </div>
              )}
            </div>
            {displayTarget && (
              <div
                className="text-xs font-mono text-muted-foreground truncate"
                title={displayTarget}
              >
                {displayTarget}
              </div>
            )}
          </div>

          {/* View Diff button (if diff available) */}
          {toolCall.diffId && onViewDiff && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onViewDiff(toolCall.diffId!);
              }}
              className="flex-shrink-0 text-xs text-primary hover:text-primary/80 hover:underline font-medium transition-colors px-2 py-1 rounded hover:bg-primary/10"
              title="View file changes"
            >
              View Diff
            </button>
          )}

          {/* Status indicator */}
          <div className="flex-shrink-0">{getStatusIndicator()}</div>

          {/* Timestamp */}
          <span className="text-xs text-muted-foreground flex-shrink-0">
            {formatTimestamp(toolCall.timestamp)}
          </span>
        </div>

        {/* Expanded details */}
        <AnimatePresence>
          {expanded && hasDetails && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="overflow-hidden"
            >
              <div className="border-t border-border bg-muted/20 px-4 py-3">
                <ToolDetails toolCall={toolCall} />
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

/**
 * Detailed view for expanded tool call
 */
function ToolDetails({ toolCall }: { toolCall: ToolCallEntry }) {
  const { actionType } = toolCall;
  const displayPath = formatLogPathForDisplay(
    actionType.file_path || actionType.path || 'N/A'
  );
  const displayCommand = formatShellCommandForDisplay(
    String((actionType as any).command || actionType.target || 'N/A')
  );

  // Render details based on action type
  switch (actionType.action) {
    case 'command_run':
      return (
        <div className="space-y-2">
          <div className="text-xs font-medium text-muted-foreground">Command</div>
          <pre className="text-xs bg-background border border-border rounded px-3 py-2 overflow-x-auto">
            {displayCommand}
          </pre>
        </div>
      );

    case 'file_edit':
      return (
        <div className="space-y-2">
          <div className="text-xs font-medium text-muted-foreground">File Path</div>
          <div className="text-xs font-mono bg-background border border-border rounded px-3 py-2 break-all">
            {displayPath}
          </div>
          {(actionType as any).changes && (
            <>
              <div className="text-xs font-medium text-muted-foreground mt-3">Changes</div>
              <pre className="text-xs bg-background border border-border rounded px-3 py-2 overflow-x-auto max-h-48">
                {JSON.stringify((actionType as any).changes, null, 2)}
              </pre>
            </>
          )}
        </div>
      );

    case 'search':
      return (
        <div className="space-y-2">
          <div className="text-xs font-medium text-muted-foreground">Query</div>
          <div className="text-xs bg-background border border-border rounded px-3 py-2">
            {(actionType as any).query || actionType.target || 'N/A'}
          </div>
        </div>
      );

    default:
      return (
        <div className="text-xs text-muted-foreground">
          <pre className="overflow-x-auto">
            {JSON.stringify(actionType, null, 2)}
          </pre>
        </div>
      );
  }
}
