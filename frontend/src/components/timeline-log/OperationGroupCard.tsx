import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, ChevronDown, Eye, Edit, Search } from 'lucide-react';
import type { OperationGroup, ToolCallEntry } from '@/types/timeline-log';
import { cn } from '@/lib/utils';
import { formatTimestamp } from '@/utils/formatters';

interface OperationGroupCardProps {
  group: OperationGroup;
}

/**
 * Collapsible card for aggregated operations (3+ consecutive Read/Grep/Glob).
 * Shows summary with first 3 files and expands to show all operations.
 */
export function OperationGroupCard({ group }: OperationGroupCardProps) {
  const [expanded, setExpanded] = useState(false);

  // Get icon based on group type
  const getGroupIcon = () => {
    switch (group.groupType) {
      case 'file_read':
        return Eye;
      case 'file_edit':
        return Edit;
      case 'search':
        return Search;
      default:
        return Eye;
    }
  };

  // Get label based on group type
  const getGroupLabel = () => {
    switch (group.groupType) {
      case 'file_read':
        return 'File Reads';
      case 'file_edit':
        return 'File Edits';
      case 'search':
        return 'Searches';
      default:
        return 'Operations';
    }
  };

  const Icon = getGroupIcon();
  const label = getGroupLabel();

  // Get first 3 file paths for summary
  const summaryPaths = group.operations
    .slice(0, 3)
    .map(
      (op) =>
        op.actionType.file_path ||
        op.actionType.path ||
        op.actionType.target ||
        'unknown'
    );

  return (
    <div className="relative pl-12">
      {/* Timeline dot */}
      <div
        className={cn(
          'absolute left-[1.875rem] top-3 w-3 h-3 rounded-full border-2 border-background',
          group.status === 'failed'
            ? 'bg-destructive'
            : group.status === 'running'
            ? 'bg-warning'
            : 'bg-info'
        )}
        aria-hidden="true"
      />

      {/* Card */}
      <div
        className={cn(
          'border rounded-lg overflow-hidden transition-colors bg-card',
          expanded ? 'border-info/50' : 'border-border hover:border-info/30'
        )}
      >
        {/* Header */}
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full px-4 py-3 flex items-center gap-3 hover:bg-info/5 transition-colors"
        >
          {/* Expand icon */}
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-muted-foreground flex-shrink-0" />
          ) : (
            <ChevronRight className="w-4 h-4 text-muted-foreground flex-shrink-0" />
          )}

          {/* Icon with count badge */}
          <div className="relative flex-shrink-0">
            <Icon className="w-4 h-4 text-info" />
            <div className="absolute -top-2 -right-2 bg-info text-background text-xs font-bold rounded-full w-5 h-5 flex items-center justify-center">
              {group.count}
            </div>
          </div>

          {/* Summary */}
          <div className="flex-1 text-left min-w-0">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-sm font-medium text-info">{label}</span>
              <span className="text-xs text-muted-foreground">
                {formatTimestamp(group.timestamp_start)}
              </span>
            </div>
            <div className="text-xs text-muted-foreground font-mono truncate">
              {summaryPaths.join(', ')}
              {group.operations.length > 3 && ` +${group.operations.length - 3} more`}
            </div>
          </div>

          {/* Status indicator */}
          {group.status === 'failed' && (
            <div className="flex-shrink-0 w-2 h-2 bg-destructive rounded-full" />
          )}
        </button>

        {/* Expanded operations list */}
        <AnimatePresence>
          {expanded && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="overflow-hidden"
            >
              <div className="border-t border-border bg-muted/20">
                <div className="px-4 py-3 space-y-2 max-h-64 overflow-y-auto">
                  {group.operations.map((operation, index) => (
                    <OperationItem key={operation.id} operation={operation} index={index} />
                  ))}
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

/**
 * Individual operation item within expanded group
 */
function OperationItem({
  operation,
  index,
}: {
  operation: ToolCallEntry;
  index: number;
}) {
  const target =
    operation.actionType.file_path ||
    operation.actionType.path ||
    operation.actionType.target ||
    'unknown';

  return (
    <div className="flex items-start gap-3 text-xs">
      {/* Index */}
      <span className="text-muted-foreground font-mono w-6 flex-shrink-0">
        {index + 1}.
      </span>

      {/* File path */}
      <span className="flex-1 font-mono text-foreground truncate" title={target}>
        {target}
      </span>

      {/* Timestamp */}
      <span className="text-muted-foreground flex-shrink-0">
        {new Date(operation.timestamp).toLocaleTimeString()}
      </span>

      {/* Status */}
      {operation.status === 'failed' && (
        <span className="text-destructive flex-shrink-0">✕</span>
      )}
    </div>
  );
}
