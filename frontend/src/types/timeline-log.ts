/**
 * Timeline Log Type Definitions
 *
 * Type definitions for the timeline log system matching the Rust backend.
 * These types represent the structured data for displaying agent execution history.
 */

// ============================================================================
// Base Normalized Entry Types (from Rust backend)
// ============================================================================

/**
 * Represents multiple similar actions aggregated together for cleaner display.
 * Used when multiple operations of the same type occur in sequence.
 */
export interface AggregatedAction {
  type: 'AggregatedAction';
  tool_name: string;
  action: string;
  operations: ActionOperation[];
  start_line: number;
  end_line: number;
  timestamp_start: string;
  timestamp_end: string;
  total_count: number;
}

/**
 * Individual operation within an aggregated action.
 */
export interface ActionOperation {
  target?: string;
  line_number: number;
  timestamp: string;
}

/**
 * Represents spawning of a child subagent thread.
 */
export interface SubagentSpawn {
  type: 'SubagentSpawn';
  child_attempt_id: string;
  task_description: string;
  tool_use_id: string;
  timestamp: string;
  line_number: number;
}

/**
 * Single action performed by a tool.
 */
export interface ActionType {
  type: 'Action';
  tool_name: string;
  action: string;
  target?: string;
  timestamp: string;
  line_number: number;
}

/**
 * File system change event.
 */
export interface FileChange {
  type: 'FileChange';
  path: string;
  change_type: 'Created' | 'Modified' | 'Deleted' | { Renamed: { from: string } };
  lines_added?: number;
  lines_removed?: number;
  timestamp: string;
  line_number: number;
}

/**
 * Todo list item state.
 */
export interface TodoItem {
  type: 'TodoItem';
  status: 'Pending' | 'InProgress' | 'Completed';
  content: string;
  timestamp: string;
  line_number: number;
}

/**
 * Tool execution status.
 */
export interface ToolStatus {
  type: 'ToolStatus';
  tool_name: string;
  status: 'Success' | 'Failed' | 'Cancelled';
  error_message?: string;
  timestamp: string;
  line_number: number;
}

/**
 * Union type of all possible normalized entry types from the backend.
 */
export type NormalizedEntry =
  | ActionType
  | AggregatedAction
  | SubagentSpawn
  | FileChange
  | TodoItem
  | ToolStatus;

// ============================================================================
// Timeline Display Types
// ============================================================================

/**
 * Base interface for all timeline entries.
 * Extended by specific entry types for the UI layer.
 */
export interface TimelineEntry {
  id: string;
  type:
    | 'tool_call'
    | 'operation_group'
    | 'subagent'
    | 'user_message'
    | 'assistant_message'
    | 'file_change'
    | 'thinking'
    | 'error'
    | 'loading';
  timestamp: string;
  [key: string]: any; // Additional type-specific fields
}

/**
 * Synthetic loading entry (frontend-only).
 * Mirrors Vibe Kanban: show a pulsing "agent is running" row while the process is active.
 */
export interface LoadingEntry extends TimelineEntry {
  type: 'loading';
}

/**
 * Grouped operations of the same type for cleaner timeline display.
 * E.g., multiple file reads grouped together.
 */
export interface OperationGroup extends TimelineEntry {
  type: 'operation_group';
  groupType: 'file_read' | 'search' | 'file_edit';
  operations: ToolCallEntry[];
  count: number;
  timestamp_start: string;
  timestamp_end: string;
  status: 'running' | 'success' | 'failed';
}

/**
 * Individual tool call entry in the timeline.
 */
export interface ToolCallEntry extends TimelineEntry {
  type: 'tool_call';
  toolName: string;
  actionType: {
    action: string;
    file_path?: string;
    path?: string;
    target?: string;
    command?: string;
    query?: string;
    url?: string;
    todos?: unknown;
    operation?: string;
    description?: string;
    plan?: string;
    arguments?: unknown;
    result?: unknown;
    changes?: unknown;
  };
  status?:
    | 'created'
    | 'running'
    | 'pending_approval'
    | 'success'
    | 'failed'
    | 'denied'
    | 'timed_out'
    | 'cancelled';
  statusReason?: string;
  approvalId?: string;
  duration?: number;
  diffStats?: {
    additions: number;
    deletions: number;
  };
  diffId?: string;
}

/**
 * Subagent execution thread with nested timeline entries.
 */
export interface SubagentThread {
  id: string;
  parentAttemptId: string;
  agentName: string;
  taskDescription: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  depth: number;
  entries: TimelineEntry[];
  startedAt: string;
  completedAt?: string;
}

/**
 * Subagent spawn entry in the timeline.
 */
export interface SubagentEntry extends TimelineEntry {
  type: 'subagent';
  thread: SubagentThread;
}

/**
 * User message entry in the timeline.
 */
export interface UserMessageEntry extends TimelineEntry {
  type: 'user_message';
  content: string;
}

/**
 * Assistant message entry in the timeline.
 */
export interface AssistantMessageEntry extends TimelineEntry {
  type: 'assistant_message';
  content: string;
}

/**
 * File change entry in the timeline.
 */
export interface FileChangeEntry extends TimelineEntry {
  type: 'file_change';
  path: string;
  changeType: 'Created' | 'Modified' | 'Deleted' | { Renamed: { from: string } };
  linesAdded?: number;
  linesRemoved?: number;
  diffId?: string;
}

/**
 * Thinking/reasoning entry in the timeline.
 */
export interface ThinkingEntry extends TimelineEntry {
  type: 'thinking';
  content: string;
}

/**
 * Error entry in the timeline.
 */
export interface ErrorEntry extends TimelineEntry {
  type: 'error';
  error: string;
  tool?: string;
}

/**
 * Token usage telemetry extracted from normalized `token_usage_info` entries.
 * Displayed separately from the conversation timeline.
 */
export interface TimelineTokenUsageInfo {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  modelContextWindow?: number;
}

// ============================================================================
// API Response Types
// ============================================================================

/**
 * File diff summary from backend.
 */
export interface FileDiffSummary {
  id: string;
  file_path: string;
  additions: number;
  deletions: number;
  change_type: 'created' | 'modified' | 'deleted';
}

/**
 * Response from structured logs API endpoint.
 */
export interface StructuredLogsResponse {
  entries: NormalizedEntry[];
  total: number;
  page: number;
  page_size: number;
  file_diffs: FileDiffSummary[];
}

/**
 * Node in the subagent tree hierarchy.
 */
export interface SubagentTreeNode {
  attempt_id: string;
  status: string;
  started_at?: string;
  completed_at?: string;
  depth: number;
  children: SubagentTreeNode[];
}

/**
 * Response from subagent tree API endpoint.
 */
export interface SubagentTreeResponse {
  nodes: SubagentTreeNode[];
  total_count: number;
}

// ============================================================================
// WebSocket Message Types
// ============================================================================

/**
 * User message event from WebSocket.
 */
export interface UserMessageEvent {
  type: 'UserMessage';
  attempt_id: string;
  content: string;
  timestamp: string;
}

/**
 * Log event from WebSocket.
 */
export interface LogEvent {
  type: 'Log';
  attempt_id: string;
  log_type: string;
  content: string;
  timestamp: string;
  /** Unique identifier for this log entry */
  id?: string;
  /** When the log was created (for ordering) */
  created_at?: string;
  /** Tool name if this is a tool-related log */
  tool_name?: string;
}

/**
 * Status update event from WebSocket.
 */
export interface StatusEvent {
  type: 'Status';
  attempt_id: string;
  status: string;
  timestamp: string;
}

/**
 * Approval request event from WebSocket.
 */
export interface ApprovalRequestEvent {
  type: 'ApprovalRequest';
  attempt_id: string;
  tool_use_id: string;
  tool_name: string;
  tool_input: any;
  timestamp: string;
}

/**
 * Union type of all possible agent events from WebSocket.
 */
export type AgentEvent =
  | LogEvent
  | StatusEvent
  | ApprovalRequestEvent
  | UserMessageEvent;

/**
 * Client message sent through WebSocket.
 */
export interface ClientMessage {
  type: 'UserInput';
  content: string;
}

// ============================================================================
// Utility Types
// ============================================================================

/**
 * Status of an agent execution.
 */
export type AgentStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

/**
 * Tool execution status.
 */
export type ToolExecutionStatus = 'running' | 'success' | 'failed';

/**
 * File change type.
 */
export type FileChangeType = 'Created' | 'Modified' | 'Deleted' | { Renamed: { from: string } };

/**
 * Todo item status.
 */
export type TodoStatus = 'Pending' | 'InProgress' | 'Completed';
