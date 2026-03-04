import type { NormalizedEntry } from '@/bindings/NormalizedEntry';
import { UserMessage } from './UserMessage';
import { AssistantMessage } from './AssistantMessage';
import { SystemMessage } from './SystemMessage';
import { ErrorMessage } from './ErrorMessage';
import { ThinkingEntry } from './ThinkingEntry';
import { LoadingCard } from './LoadingCard';
import { NextActionCard } from './NextActionCard';
import { ToolCallCard } from './ToolCallCard';
import { PendingApprovalEntry } from './PendingApprovalEntry';
import { logger } from '@/lib/logger';

interface DisplayConversationEntryProps {
  entry: NormalizedEntry;
}

/**
 * Routes normalized entries to appropriate component renderer.
 * Handles 9+ entry types with consistent styling and spacing.
 */
export function DisplayConversationEntry({
  entry,
}: DisplayConversationEntryProps) {
  const { entry_type, content, timestamp } = entry;

  // Validate entry has entry_type
  if (!entry_type || !entry_type.type) {
    logger.warn('Entry missing entry_type:', entry);
    return (
      <div className="p-4 bg-muted/50 border border-border rounded text-sm text-muted-foreground">
        <p className="font-medium">Invalid entry format</p>
        <p className="text-xs mt-1">Entry is missing required entry_type property</p>
      </div>
    );
  }

  try {
    switch (entry_type.type) {
      case 'user_message':
        return <UserMessage content={content} timestamp={timestamp} />;

      case 'user_feedback':
        return (
          <UserMessage
            content={content}
            deniedTool={entry_type.denied_tool}
            timestamp={timestamp}
          />
        );

      case 'assistant_message':
        return <AssistantMessage content={content} timestamp={timestamp} />;

      case 'system_message':
        return <SystemMessage content={content} timestamp={timestamp} />;

      case 'error_message':
        return (
          <ErrorMessage
            content={content}
            errorType={entry_type.error_type}
            timestamp={timestamp}
          />
        );

      case 'thinking':
        return <ThinkingEntry content={content} timestamp={timestamp} />;

      case 'loading':
        return <LoadingCard timestamp={timestamp} />;

      case 'next_action':
        return (
          <NextActionCard
            failed={entry_type.failed}
            executionProcesses={entry_type.execution_processes}
            needsSetup={entry_type.needs_setup}
            timestamp={timestamp}
          />
        );

      case 'tool_use':
        // Check if pending approval
        if (entry_type.status.status === 'pending_approval') {
          return (
            <PendingApprovalEntry
              toolName={entry_type.tool_name}
              actionType={entry_type.action_type}
              status={entry_type.status}
              content={content}
              timestamp={timestamp}
            />
          );
        }

        // Render completed tool call
        return (
          <ToolCallCard
            toolName={entry_type.tool_name}
            actionType={entry_type.action_type}
            status={entry_type.status}
            content={content}
            timestamp={timestamp}
          />
        );

      default:
        // Exhaustive check - TypeScript will error if new types added
        const _exhaustive: never = entry_type;
        return _exhaustive;
    }
  } catch (error) {
    logger.error('Error rendering entry:', { entry, error });
    return (
      <div className="p-4 bg-destructive/10 border border-destructive/20 rounded text-sm text-destructive">
        <p className="font-medium">Failed to render entry</p>
        <p className="text-xs text-destructive/70 mt-1">
          {error instanceof Error ? error.message : 'Unknown error'}
        </p>
      </div>
    );
  }
}
