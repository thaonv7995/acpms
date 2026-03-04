/**
 * Agent Session Components - Index file
 * Re-exports all components for the Agent Terminal UI
 */

// Main panel
export { AgentSessionPanel } from './AgentSessionPanel';
export { RightPanelContainer } from './RightPanelContainer';

// Sub-components
export { LogStreamDisplay } from './LogStreamDisplay';
export { LogEntry } from './LogEntry';
export { SummaryActions } from './SummaryActions';
export { ChatInput } from './ChatInput';
export { MentionPopover } from './MentionPopover';
export { PanelHeader } from './PanelHeader';
export { TerminalHeader } from './TerminalHeader';

// Log entry types
export * from './log-entries';

// Hooks
export { useAgentSession } from './useAgentSession';

// Types
export type {
  LogEntryType,
  AgentLogEntry,
  LogEntryMetadata,
  DiffSummary,
  AgentSessionState,
  MentionItem,
} from './types';

// Utilities
export { mapLogType, detectLogType, extractFileInfo } from './types';
export * from './utils';
