// Attempt Log Types for real-time log streaming

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export type LogType =
  | 'system'      // System messages (agent start/stop)
  | 'thinking'    // AI thinking/reasoning
  | 'tool_call'   // Tool invocations
  | 'tool_result' // Tool results
  | 'output'      // General output
  | 'error'       // Error messages
  | 'permission'  // Permission requests
  | 'user_input'; // User follow-up messages

export interface AttemptLog {
  id: string;
  attempt_id: string;
  timestamp: string;
  level: LogLevel;
  type: LogType;
  message: string;
  metadata?: LogMetadata;
}

export interface LogMetadata {
  // Tool call metadata
  tool_name?: string;
  tool_input?: Record<string, unknown>;
  tool_output?: string;
  tool_duration_ms?: number;

  // Error metadata
  error_code?: string;
  stack_trace?: string;

  // Permission metadata
  permission_type?: string;
  permission_status?: 'pending' | 'approved' | 'denied';

  // File reference
  file_path?: string;
  line_number?: number;

  // Diff reference
  diff_id?: string;
}

export interface LogsFilter {
  level?: LogLevel[];
  type?: LogType[];
  search?: string;
}

export interface LogsResponse {
  logs: AttemptLog[];
  total: number;
  has_more: boolean;
  cursor?: string;
}

// WebSocket message types
export interface LogStreamMessage {
  type: 'log' | 'batch' | 'clear' | 'error';
  payload: AttemptLog | AttemptLog[] | { error: string };
}

// Log display configuration
export const logLevelConfig: Record<LogLevel, { color: string; icon: string }> = {
  debug: { color: 'text-slate-500', icon: 'bug_report' },
  info: { color: 'text-blue-400', icon: 'info' },
  warn: { color: 'text-yellow-400', icon: 'warning' },
  error: { color: 'text-red-400', icon: 'error' },
};

export const logTypeConfig: Record<LogType, { color: string; label: string }> = {
  system: { color: 'text-purple-400', label: 'SYS' },
  thinking: { color: 'text-cyan-400', label: 'THINK' },
  tool_call: { color: 'text-green-400', label: 'TOOL' },
  tool_result: { color: 'text-emerald-400', label: 'RESULT' },
  output: { color: 'text-slate-300', label: 'OUT' },
  error: { color: 'text-red-400', label: 'ERR' },
  permission: { color: 'text-orange-400', label: 'PERM' },
  user_input: { color: 'text-blue-300', label: 'USER' },
};
