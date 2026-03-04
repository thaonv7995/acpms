import { useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import type { ActionType } from '@/bindings/ActionType';
import type { ToolStatus } from '@/bindings/ToolStatus';
import { BaseEntry } from './BaseEntry';
import { StatusIndicator } from './StatusIndicator';
import { getActionIcon } from '@/utils/icon-mapping';
import { ToolContent } from './tool-call-content';

interface ToolCallCardProps {
  toolName: string;
  actionType: ActionType;
  status: ToolStatus;
  content: string;
  timestamp?: string | null;
}

/**
 * Extract diff stats from content string (e.g., "+4 -0")
 */
function extractDiffStats(content: string): { additions?: number; deletions?: number } {
  const match = content.match(/\+(\d+)\s+-(\d+)/);
  if (match) {
    return {
      additions: parseInt(match[1], 10),
      deletions: parseInt(match[2], 10),
    };
  }
  return {};
}

/**
 * Render inline tool summary for vibe-kanban style display.
 * Format: <icon> /path/to/file.md +4 -0
 */
function InlineToolDisplay({ actionType, content }: { actionType: ActionType; content: string }) {
  const { additions, deletions } = extractDiffStats(content);

  switch (actionType.action) {
    case 'file_read':
      return (
        <span className="font-mono text-sm text-secondary-foreground">
          {actionType.path}
        </span>
      );

    case 'file_edit':
      return (
        <span className="font-mono text-sm text-secondary-foreground">
          {actionType.path}
          {(additions !== undefined || deletions !== undefined) && (
            <>
              {' '}
              <span className="text-green-500">+{additions ?? 0}</span>
              {' '}
              <span className="text-red-500">-{deletions ?? 0}</span>
            </>
          )}
        </span>
      );

    case 'command_run':
      return (
        <span className="font-mono text-sm text-secondary-foreground truncate">
          {actionType.command.slice(0, 80)}
          {actionType.command.length > 80 ? '...' : ''}
        </span>
      );

    case 'search':
      return (
        <span className="text-sm text-secondary-foreground truncate">
          {actionType.query}
        </span>
      );

    case 'web_fetch':
      return (
        <span className="text-sm text-secondary-foreground truncate">
          {actionType.url}
        </span>
      );

    case 'tool':
      return (
        <span className="text-sm text-secondary-foreground">
          {actionType.tool_name}
        </span>
      );

    default:
      return (
        <span className="text-sm text-secondary-foreground truncate">
          {content.slice(0, 100)}
        </span>
      );
  }
}

/**
 * Tool call card in vibe-kanban style.
 * Compact single-line display with optional expansion.
 */
export function ToolCallCard({
  actionType,
  status,
  content,
  timestamp,
}: ToolCallCardProps) {
  const [expanded, setExpanded] = useState(false);
  const Icon = getActionIcon(actionType.action);

  // Determine if this tool has expandable content
  const hasExpandableContent =
    actionType.action === 'command_run' ||
    actionType.action === 'file_edit' ||
    (actionType.action === 'tool' && actionType.result);

  return (
    <BaseEntry variant="action" timestamp={timestamp}>
      {/* Compact single-line display like vibe-kanban */}
      <div
        className={`flex items-center gap-2 ${hasExpandableContent ? 'cursor-pointer' : ''}`}
        onClick={() => hasExpandableContent && setExpanded(!expanded)}
      >
        {/* Tool icon */}
        <Icon className="w-3.5 h-3.5 text-secondary-foreground flex-shrink-0" />

        {/* Inline tool display */}
        <div className="flex-1 min-w-0 overflow-hidden">
          <InlineToolDisplay actionType={actionType} content={content} />
        </div>

        {/* Expand indicator for expandable tools */}
        {hasExpandableContent && (
          <div className="flex-shrink-0">
            {expanded ? (
              <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5 text-muted-foreground" />
            )}
          </div>
        )}

        {/* Status indicator (only show for non-success) */}
        {status.status !== 'success' && (
          <StatusIndicator status={status.status} />
        )}
      </div>

      {/* Expanded details section */}
      {expanded && hasExpandableContent && (
        <div className="mt-2 ml-5 border-l border-border pl-3 max-h-48 overflow-y-auto">
          <ToolContent actionType={actionType} content={content} />
        </div>
      )}
    </BaseEntry>
  );
}
