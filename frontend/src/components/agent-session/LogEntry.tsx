/**
 * LogEntry - Routes to the correct log entry component based on type
 * Main entry point for rendering individual log entries
 */

import { memo } from 'react';
import type { AgentLogEntry } from './types';
import {
  SystemLogEntry,
  CommandLogEntry,
  FileReadEntry,
  FileWriteEntry,
  AgentResponseEntry,
  UserInputEntry,
  ErrorLogEntry,
  OutputLogEntry,
  ThinkingLogEntry,
  ToolCallEntry,
} from './log-entries';

interface LogEntryProps {
  entry: AgentLogEntry;
  onFileClick?: (filepath: string) => void;
}

export const LogEntry = memo(function LogEntry({ entry, onFileClick }: LogEntryProps) {
  switch (entry.type) {
    case 'system':
      return <SystemLogEntry entry={entry} />;

    case 'command':
      return <CommandLogEntry entry={entry} />;

    case 'file_read':
      return <FileReadEntry entry={entry} onFileClick={onFileClick} />;

    case 'file_write':
      return <FileWriteEntry entry={entry} onFileClick={onFileClick} />;

    case 'agent':
      return <AgentResponseEntry entry={entry} />;

    case 'user_input':
      return <UserInputEntry entry={entry} />;

    case 'error':
      return <ErrorLogEntry entry={entry} />;

    case 'output':
      return <OutputLogEntry entry={entry} />;

    case 'thinking':
      return <ThinkingLogEntry entry={entry} />;

    case 'tool_call':
    case 'tool_result':
      return <ToolCallEntry entry={entry} />;

    case 'summary':
      // Summary is handled separately in SummaryActions
      return null;

    default:
      // Fallback to output style
      return <OutputLogEntry entry={entry} />;
  }
});
