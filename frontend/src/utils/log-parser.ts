// log-parser.ts - Parse raw log content into structured entries for display
import type { NormalizedEntry } from '@/bindings/NormalizedEntry';
import type { NormalizedEntryType } from '@/bindings/NormalizedEntryType';
import type { ActionType } from '@/bindings/ActionType';
import type { ToolStatus } from '@/bindings/ToolStatus';

/**
 * Parsed tool call information from raw log content
 */
interface ParsedToolCall {
  toolName: string;
  actionType: ActionType;
  path?: string;
  additions?: number;
  deletions?: number;
}

// Regex patterns for detecting tool calls in raw logs
const PATTERNS = {
  // File read: "Read /path/to/file" or "◎ /path/to/file"
  fileRead: /^(?:Read|◎)\s+(.+\.(?:ts|tsx|js|jsx|json|md|rs|py|go|css|html|yml|yaml|toml|sql|sh|env|gitignore)[^\s]*)/m,

  // File edit: "Edit /path/to/file +X -Y" or "☐ /path/to/file +X -Y"
  fileEdit: /^(?:Edit|Write|☐)\s+(.+\.(?:ts|tsx|js|jsx|json|md|rs|py|go|css|html|yml|yaml|toml|sql|sh|env|gitignore)[^\s]*)\s*(?:\+(\d+)\s*-(\d+))?/m,

  // Bash command: "$ command" or "bash: command"
  bashCommand: /^(?:\$|bash:|Bash:?)\s+(.+)/m,

  // Glob search: "Glob pattern" or "Search: pattern"
  globSearch: /^(?:Glob|Grep|Search):?\s+(.+)/m,

  // Tool call generic: "Using tool: ToolName"
  genericTool: /^Using tool:\s+(\w+)(?:\s+(.+))?/m,

  // File change summary: "Modified: path (+X, -Y)"
  fileChangeSummary: /^(Created|Modified|Deleted):\s+([^\s]+)(?:\s+\(\+(\d+),\s*-(\d+)\))?/m,
};

/**
 * Try to parse a tool call from raw log content
 */
function parseToolCall(content: string): ParsedToolCall | null {
  // Try file read pattern
  const readMatch = content.match(PATTERNS.fileRead);
  if (readMatch) {
    return {
      toolName: 'Read',
      actionType: { action: 'file_read', path: readMatch[1] } as ActionType,
      path: readMatch[1],
    };
  }

  // Try file edit pattern
  const editMatch = content.match(PATTERNS.fileEdit);
  if (editMatch) {
    return {
      toolName: 'Edit',
      actionType: {
        action: 'file_edit',
        path: editMatch[1],
        changes: [],
      } as ActionType,
      path: editMatch[1],
      additions: editMatch[2] ? parseInt(editMatch[2], 10) : undefined,
      deletions: editMatch[3] ? parseInt(editMatch[3], 10) : undefined,
    };
  }

  // Try bash command pattern
  const bashMatch = content.match(PATTERNS.bashCommand);
  if (bashMatch) {
    return {
      toolName: 'Bash',
      actionType: {
        action: 'command_run',
        command: bashMatch[1],
        result: null,
      } as ActionType,
    };
  }

  // Try glob/search pattern
  const searchMatch = content.match(PATTERNS.globSearch);
  if (searchMatch) {
    return {
      toolName: 'Search',
      actionType: {
        action: 'search',
        query: searchMatch[1],
      } as ActionType,
    };
  }

  // Try generic tool pattern
  const toolMatch = content.match(PATTERNS.genericTool);
  if (toolMatch) {
    const toolName = toolMatch[1];
    const target = toolMatch[2];

    // Map to specific action types
    if (toolName.toLowerCase() === 'read' && target) {
      return {
        toolName: 'Read',
        actionType: { action: 'file_read', path: target } as ActionType,
        path: target,
      };
    }
    if (toolName.toLowerCase() === 'edit' && target) {
      return {
        toolName: 'Edit',
        actionType: { action: 'file_edit', path: target, changes: [] } as ActionType,
        path: target,
      };
    }

    return {
      toolName,
      actionType: {
        action: 'tool',
        tool_name: toolName,
        arguments: target ? { target } : null,
        result: null,
      } as ActionType,
    };
  }

  // Try file change summary
  const changeMatch = content.match(PATTERNS.fileChangeSummary);
  if (changeMatch) {
    const action = changeMatch[1].toLowerCase();
    const path = changeMatch[2];

    if (action === 'modified' || action === 'created') {
      return {
        toolName: action === 'created' ? 'Write' : 'Edit',
        actionType: {
          action: 'file_edit',
          path,
          changes: [],
        } as ActionType,
        path,
        additions: changeMatch[3] ? parseInt(changeMatch[3], 10) : undefined,
        deletions: changeMatch[4] ? parseInt(changeMatch[4], 10) : undefined,
      };
    }
  }

  return null;
}

/**
 * Convert a parsed tool call to a NormalizedEntry
 */
function toolCallToNormalizedEntry(
  toolCall: ParsedToolCall,
  content: string,
  timestamp: string | null
): NormalizedEntry {
  const status: ToolStatus = { status: 'success' };

  // Format content for display
  let displayContent = toolCall.path || content;
  if (toolCall.additions !== undefined || toolCall.deletions !== undefined) {
    const adds = toolCall.additions ?? 0;
    const dels = toolCall.deletions ?? 0;
    displayContent = `${toolCall.path} +${adds} -${dels}`;
  }

  const entryType: NormalizedEntryType = {
    type: 'tool_use',
    tool_name: toolCall.toolName,
    action_type: toolCall.actionType,
    status,
  };

  return {
    timestamp,
    entry_type: entryType,
    content: displayContent,
  };
}

/**
 * Parse raw log content and convert to NormalizedEntry
 * Returns the original content as assistant_message if no tool call detected
 */
export function parseLogContent(
  content: string,
  timestamp: string | null
): NormalizedEntry {
  // Try to parse as tool call
  const toolCall = parseToolCall(content);
  if (toolCall) {
    return toolCallToNormalizedEntry(toolCall, content, timestamp);
  }

  // Default to assistant message
  return {
    timestamp,
    entry_type: { type: 'assistant_message' },
    content,
  };
}

/**
 * Check if content looks like a system message
 */
export function isSystemMessage(content: string): boolean {
  const systemPatterns = [
    /^Starting/i,
    /^Spawning/i,
    /^Agent\s+spawned/i,
    /^Claude\s+agent/i,
    /^Initializing/i,
    /^Loading/i,
    /^Completed/i,
    /^Failed/i,
    /^Error:/i,
  ];

  return systemPatterns.some((pattern) => pattern.test(content.trim()));
}

/**
 * Parse multiple log lines and merge consecutive messages
 */
export function parseLogLines(
  lines: Array<{ content: string; timestamp: string | null; type: string }>
): NormalizedEntry[] {
  const entries: NormalizedEntry[] = [];
  let currentMessage: string[] = [];
  let currentTimestamp: string | null = null;

  const flushMessage = () => {
    if (currentMessage.length > 0) {
      const content = currentMessage.join('');
      if (content.trim()) {
        const parsed = parseLogContent(content, currentTimestamp);
        entries.push(parsed);
      }
      currentMessage = [];
      currentTimestamp = null;
    }
  };

  for (const line of lines) {
    const content = line.content;

    // Check if this is a tool call line
    const toolCall = parseToolCall(content);
    if (toolCall) {
      // Flush any pending message
      flushMessage();
      // Add tool call entry
      entries.push(toolCallToNormalizedEntry(toolCall, content, line.timestamp));
      continue;
    }

    // Check if this is a system message
    if (isSystemMessage(content)) {
      // Flush any pending message
      flushMessage();
      // Add system message
      entries.push({
        timestamp: line.timestamp,
        entry_type: { type: 'system_message' },
        content,
      });
      continue;
    }

    // Accumulate regular content
    if (currentMessage.length === 0) {
      currentTimestamp = line.timestamp;
    }
    currentMessage.push(content);
  }

  // Flush remaining message
  flushMessage();

  return entries;
}
