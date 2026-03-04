/**
 * Types for Agent Session components
 */

export type LogEntryType =
  | 'system'      // Gray background, System: prefix
  | 'command'     // Green dot indicator, monospace font
  | 'output'      // Indented, gray text
  | 'file_read'   // File icon + filename
  | 'file_write'  // Edit icon + filename + diff stats (+N -M)
  | 'agent'       // Normal text, markdown rendering
  | 'user_input'  // Blue background, user message
  | 'error'       // Red background
  | 'thinking'    // Italic text, thinking indicator
  | 'tool_call'   // Tool invocation
  | 'tool_result' // Tool result
  | 'summary';    // Collapsible card with actions

export interface AgentLogEntry {
  id: string;
  type: LogEntryType;
  content: string;
  timestamp?: string;
  metadata?: LogEntryMetadata;
}

export interface LogEntryMetadata {
  // File operations
  filepath?: string;
  additions?: number;
  deletions?: number;
  is_new_file?: boolean;
  lines?: number;

  // Command execution
  command?: string;
  output?: string;
  exit_code?: number;
  duration_ms?: number;

  // Tool calls
  tool_name?: string;
  tool_input?: string | Record<string, unknown>;
  tool_output?: string;

  // Error info
  error_code?: string;
  stack_trace?: string;

  // System info
  model?: string;

  // References
  diff_id?: string;
  attempt_id?: string;
}

export interface DiffSummary {
  filesChanged: number;
  additions: number;
  deletions: number;
  filesAdded?: number;
  filesModified?: number;
  filesDeleted?: number;
}

export interface AgentSessionState {
  status: 'idle' | 'running' | 'completed' | 'failed' | 'cancelled' | 'waiting_input';
  attemptId?: string;
  branch?: string;
  logs: AgentLogEntry[];
  diffSummary?: DiffSummary;
  error?: string;
}

export interface MentionItem {
  type: 'file' | 'task' | 'requirement';
  value: string;
  display: string;
  icon: string;
}

// Map backend log types to our display types
export function mapLogType(backendType: string): LogEntryType {
  const typeMap: Record<string, LogEntryType> = {
    system: 'system',
    thinking: 'thinking',
    tool_call: 'tool_call',
    tool_result: 'tool_result',
    output: 'output',
    error: 'error',
    permission: 'system',
    user_input: 'user_input',
    // Legacy/alternate names
    stderr: 'error',
    input: 'user_input',
  };
  return typeMap[backendType] || 'output';
}

// Parse log content to detect specific entry types
export function detectLogType(content: string, currentType: LogEntryType): LogEntryType {
  // Detect file read patterns
  if (content.match(/^(Read|Reading|Opened|Viewing|📄)\s+[\w./-]+/i)) {
    return 'file_read';
  }

  // Detect file write patterns
  if (content.match(/^(Write|Writing|Edited|Modified|Created|Updated|✏️)\s+[\w./-]+/i)) {
    return 'file_write';
  }

  // Detect command patterns
  if (content.match(/^(\$|>|#)\s+\S+/) || content.match(/^(Running|Executing):\s+/i)) {
    return 'command';
  }

  return currentType;
}

// Extract file info from content
export function extractFileInfo(content: string): { filepath: string; additions?: number; deletions?: number } | null {
  // Pattern: filename +N -M
  const diffMatch = content.match(/^([\w./-]+)\s+\+(\d+)\s+-(\d+)/);
  if (diffMatch) {
    return {
      filepath: diffMatch[1],
      additions: parseInt(diffMatch[2], 10),
      deletions: parseInt(diffMatch[3], 10),
    };
  }

  // Pattern: just filepath
  const fileMatch = content.match(/^(?:📄|✏️)?\s*([\w./-]+(?:\.\w+)?)/);
  if (fileMatch) {
    return { filepath: fileMatch[1] };
  }

  return null;
}
