/**
 * Phase 2: Single transform layer - Raw log → TimelineEntry[]
 * R2: Prefer log_type=normalized from backend (format: {entry_type, content, timestamp}).
 * Fallbacks: simple (user/stderr), stdout/process_stdout (legacy), legacy (tool_call/file_change).
 */
import type { TimelineEntry } from '@/types/timeline-log';
import {
  mapStdoutToolAction,
  inferToolAction,
  inferToolTarget,
  normalizeToolStatus,
  parseCodexJsonStdoutEntries,
  parseStdoutTranscriptEntries,
} from '@/hooks/timeline-parsers';

export type { TimelineEntry };

export interface AgentLogLike {
  id?: string;
  attempt_id?: string;
  log_type?: string;
  content?: string;
  timestamp?: string;
  created_at?: string;
  tool_name?: string | null;
  status?: unknown;
  duration?: number;
  [key: string]: unknown;
}

const INTERNAL_ACPMS_CONTRACT_PATH_REGEX =
  /(?:^|\/)\.acpms\/(?:[^/]+-output\.json|import-analysis\.json)$/i;

function ts(log: AgentLogLike, fallback?: string): string {
  return log.timestamp || log.created_at || fallback || new Date().toISOString();
}

function normalizeContractPath(path: string): string {
  return path.replace(/\\/g, '/').trim();
}

function isHiddenAcpmsContractPath(path: string | undefined): boolean {
  if (typeof path !== 'string' || !path.trim()) return false;
  return INTERNAL_ACPMS_CONTRACT_PATH_REGEX.test(normalizeContractPath(path));
}

function isHiddenInternalFileAction(action: string | undefined, path: string | undefined): boolean {
  if (action !== 'file_read' && action !== 'file_edit' && action !== 'file_write') {
    return false;
  }
  return isHiddenAcpmsContractPath(path);
}

function containsHiddenAcpmsContractPath(text: string): boolean {
  return INTERNAL_ACPMS_CONTRACT_PATH_REGEX.test(normalizeContractPath(text));
}

function isNoisyTelemetry(content: string): boolean {
  const t = (content || '').trim();
  if (!t) return true;
  return (
    t.includes('codex_otel::traces::otel_manager') ||
    t.includes('event.name="codex.sse_event"') ||
    t.includes('terminal.type=') ||
    t.includes('user.account_id=') ||
    t.startsWith('DEBUG codex_exec: Received event:')
  );
}

/** Extract user-friendly error message from Rust/tracing format. */
function extractErrorMessage(raw: string): string {
  const t = raw.trim();
  if (!t) return 'Unknown error';
  // JSON "message" field: "message":"The usage limit has been reached"
  const msgMatch = /"message"\s*:\s*"((?:[^"\\]|\\.)*)"/.exec(t);
  if (msgMatch?.[1]) {
    try {
      return msgMatch[1].replace(/\\"/g, '"').replace(/\\\\/g, '\\');
    } catch {
      return msgMatch[1];
    }
  }
  // error=http 429 Too Many Requests
  const errMatch = /error=([^:]+?)(?::\s*Some\s*\(|$)/i.exec(t);
  if (errMatch?.[1]) return errMatch[1].trim();
  // Last part after ": " (message after module path)
  const lastColon = t.lastIndexOf(': ');
  if (lastColon > 0) return t.slice(lastColon + 2).trim();
  return t;
}

/** Internal runtime logs (DEBUG/INFO/TRACE from codex/tracing) - not agent conversation. */
function isInternalRuntimeLog(content: string): boolean {
  const t = (content || '').trim();
  if (!t) return true;
  // "2026-02-28T18:20:15.999998Z DEBUG codex_core::exec_policy: ..."
  const levelMatch = /^\S+\s+(INFO|DEBUG|TRACE)\b/i.exec(t);
  if (levelMatch) return true;
  // Timestamp + module path (Rust tracing style)
  if (/^\d{4}-\d{2}-\d{2}T[\d.]+Z\s+\S+::\S+/.test(t)) return true;
  return false;
}

/** Build/compiler output (TypeScript, ESLint, etc.) - not agent conversation. */
function isBuildOutputNoise(content: string): boolean {
  const t = (content || '').trim();
  if (!t) return true;
  return (
    /Cannot find (namespace|name|module)\b/i.test(t) ||
    /\bTS\d{4}\b/.test(t) || // TypeScript error codes
    /implicitly has an ['"]any['"] type/i.test(t) ||
    /Could not find a declaration file/i.test(t) ||
    /has no exported member named/i.test(t) ||
    /This comparison appears to be unintentional/i.test(t) ||
    /Unable to save cookies for this tab/i.test(t)
  );
}

interface BreakdownTaskPayloadSpan {
  payload: Record<string, unknown>;
  start: number;
  end: number;
}

function extractBreakdownTaskPayloadSpans(content: string): BreakdownTaskPayloadSpan[] {
  const marker = 'BREAKDOWN_TASK';
  const spans: BreakdownTaskPayloadSpan[] = [];
  let cursor = 0;

  while (cursor < content.length) {
    const markerIndex = content.indexOf(marker, cursor);
    if (markerIndex < 0) break;

    const jsonStart = content.indexOf('{', markerIndex + marker.length);
    if (jsonStart < 0) {
      cursor = markerIndex + marker.length;
      continue;
    }

    let depth = 0;
    let inString = false;
    let escaped = false;
    let jsonEnd = -1;

    for (let i = jsonStart; i < content.length; i += 1) {
      const ch = content[i];
      if (inString) {
        if (escaped) {
          escaped = false;
          continue;
        }
        if (ch === '\\') {
          escaped = true;
          continue;
        }
        if (ch === '"') {
          inString = false;
        }
        continue;
      }

      if (ch === '"') {
        inString = true;
        continue;
      }
      if (ch === '{') {
        depth += 1;
        continue;
      }
      if (ch === '}') {
        depth -= 1;
        if (depth === 0) {
          jsonEnd = i;
          break;
        }
      }
    }

    if (jsonEnd < 0) {
      cursor = jsonStart + 1;
      continue;
    }

    try {
      const parsed = JSON.parse(content.slice(jsonStart, jsonEnd + 1)) as Record<string, unknown>;
      spans.push({ payload: parsed, start: markerIndex, end: jsonEnd + 1 });
    } catch {
      // Ignore malformed payload and continue scanning.
    }

    cursor = jsonEnd + 1;
  }

  return spans;
}

function normalizeBreakdownPriority(raw: unknown): string {
  const value = String(raw ?? '').trim().toLowerCase();
  if (!value) return 'medium';
  if (value === 'low' || value === 'medium' || value === 'high' || value === 'critical') {
    return value;
  }
  return 'medium';
}

function toTitleCase(raw: string): string {
  if (!raw) return raw;
  return raw
    .split(/[\s_]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join(' ');
}

function formatBreakdownTaskPayloadsAsMarkdown(payloads: Array<Record<string, unknown>>): string {
  const lines: string[] = ['Proposed Breakdown Tasks:'];
  payloads.forEach((payload, index) => {
    const title = String(payload.title ?? '').trim() || `Task ${index + 1}`;
    const description = String(payload.description ?? '').trim();
    const taskType = toTitleCase(String(payload.task_type ?? '').trim() || 'Feature');
    const priority = toTitleCase(normalizeBreakdownPriority(payload.priority));
    lines.push(`${index + 1}. **${title}**`);
    lines.push(`Type: ${taskType} · Priority: ${priority}`);
    if (description) {
      lines.push(`${description}`);
    }
  });
  return lines.join('\n');
}

export function formatBreakdownTaskContent(content: string): string | null {
  if (!content || !content.includes('BREAKDOWN_TASK')) {
    return null;
  }

  let spans = extractBreakdownTaskPayloadSpans(content);
  if (spans.length === 0 && content.includes('\\"')) {
    const unescaped = content
      .replace(/\\\\\"/g, '\\"')
      .replace(/\\"/g, '"')
      .replace(/\\n/g, '\n');
    spans = extractBreakdownTaskPayloadSpans(unescaped);
  }

  if (spans.length === 0) {
    return null;
  }

  return formatBreakdownTaskPayloadsAsMarkdown(spans.map((span) => span.payload));
}

/** 1. Normalized (R2: backend emit) - highest priority, format {entry_type:{type,...},content,timestamp} */
function handleNormalized(log: AgentLogLike, index: number): TimelineEntry[] | null {
  const lt = (log.log_type || '').toLowerCase();
  if (lt !== 'normalized') return null;

  const content = log.content || '';
  try {
    const parsed = JSON.parse(content);
    const sdkType = parsed.entry_type?.type;

    // SDK format
    if (sdkType === 'tool_use') {
      const et = parsed.entry_type;
      const toolName = et.tool_name || log.tool_name;
      const actionType = et.action_type || {};
      const statusObj = et.status || null;
      const action = inferToolAction(toolName, actionType);
      const target = inferToolTarget(action, actionType, toolName);
      const toolStatus = normalizeToolStatus(statusObj);
      const path = actionType.path ?? actionType.file_path ?? target ?? '';
      if (isHiddenInternalFileAction(action, typeof path === 'string' ? path : undefined)) {
        return [];
      }
      const entries: TimelineEntry[] = [{
        id: log.id || `tool-${index}`,
        type: 'tool_call',
        timestamp: ts(log, parsed.timestamp),
        toolName,
        actionType: {
          action,
          path: actionType.path ?? path,
          file_path: actionType.file_path ?? actionType.path ?? path,
          target: target ?? undefined,
          command: actionType.command,
          query: actionType.query,
          url: actionType.url,
          todos: actionType.todos,
          operation: actionType.operation,
          description: actionType.description,
          plan: actionType.plan,
          changes: actionType.changes,
          arguments: actionType.arguments,
          result: actionType.result,
        },
        status: toolStatus.status,
        statusReason: toolStatus.reason,
        approvalId: toolStatus.approvalId,
      }];
      // Emit file_change for file_edit/file_write so files show in timeline
      if ((action === 'file_edit' || action === 'file_write') && typeof path === 'string' && path.trim()) {
        entries.push({
          id: `${log.id || `tool-${index}`}-fc`,
          type: 'file_change',
          timestamp: ts(log, parsed.timestamp),
          path: path.trim(),
          changeType: action === 'file_write' ? 'Created' : 'Modified',
        });
      }
      return entries;
    }

    if (sdkType === 'thinking') {
      return [{
        id: log.id || `thinking-${index}`,
        type: 'thinking',
        timestamp: ts(log, parsed.timestamp),
        content: parsed.content || '',
      }];
    }

    if (sdkType === 'assistant_message' || sdkType === 'system_message') {
      const rawContent = parsed.content || '';
      const formattedBreakdown = formatBreakdownTaskContent(rawContent);
      const content = formattedBreakdown ?? rawContent;
      if (!formattedBreakdown && (isNoisyTelemetry(content) || isInternalRuntimeLog(content) || isBuildOutputNoise(content))) {
        return [];
      }
      return [{
        id: log.id || `msg-${index}`,
        type: 'assistant_message',
        timestamp: ts(log, parsed.timestamp),
        content,
        source: sdkType === 'system_message' ? 'system' : 'sdk',
      }];
    }

    if (sdkType === 'next_action') {
      const text = parsed.entry_type?.text || parsed.content || 'Next action available';
      return [{
        id: log.id || `next-action-${index}`,
        type: 'assistant_message',
        timestamp: ts(log, parsed.timestamp),
        content: text,
        source: 'sdk',
      }];
    }

    if (sdkType === 'token_usage_info') {
      if (import.meta.env.VITE_TIMELINE_SHOW_TOKEN_USAGE !== 'true') return [];
      const u = parsed.entry_type || {};
      const inT = Number(u.input_tokens ?? 0);
      const outT = Number(u.output_tokens ?? 0);
      const total = u.total_tokens != null ? Number(u.total_tokens) : inT + outT;
      return [{
        id: log.id || `token-${index}`,
        type: 'assistant_message',
        timestamp: ts(log, parsed.timestamp),
        content: `Token usage: in ${inT}, out ${outT}, total ${total}`,
        source: 'system',
      }];
    }

    if (sdkType === 'user_answered_questions') {
      const q = parsed.entry_type || {};
      const qa = q.answer ? `Q: ${q.question || 'Question'}\nA: ${q.answer}` : `Q: ${q.question || 'Question'}`;
      return [{
        id: log.id || `user-answered-${index}`,
        type: 'assistant_message',
        timestamp: ts(log, parsed.timestamp),
        content: qa,
        source: 'sdk',
      }];
    }

    if (sdkType) {
      const fallback = typeof parsed.content === 'string' && parsed.content.trim()
        ? parsed.content
        : `Agent event: ${sdkType}`;
      const formattedBreakdown = formatBreakdownTaskContent(fallback);
      const content = formattedBreakdown ?? fallback;
      if (!formattedBreakdown && (isNoisyTelemetry(content) || isInternalRuntimeLog(content) || isBuildOutputNoise(content))) {
        return [];
      }
      return [{
        id: log.id || `sdk-${index}`,
        type: 'assistant_message',
        timestamp: ts(log, parsed.timestamp),
        content,
        source: 'sdk',
      }];
    }

    // CLI format
    const tag = parsed.type;
    if (tag === 'Action') {
      const toolName = parsed.tool_name || log.tool_name;
      return [{
        id: log.id || `tool-${index}`,
        type: 'tool_call',
        timestamp: ts(log, parsed.timestamp),
        toolName,
        actionType: {
          action: mapStdoutToolAction(toolName),
          target: parsed.target,
          path: parsed.target,
          file_path: parsed.target,
        },
        status: 'success',
      }];
    }

    if (tag === 'AggregatedAction') {
      const toolName = parsed.tool_name || log.tool_name;
      return [{
        id: log.id || `group-${index}`,
        type: 'operation_group',
        timestamp: ts(log, parsed.timestamp_start),
        groupType: mapStdoutToolAction(toolName) as 'file_read' | 'search' | 'file_edit',
        operations: (parsed.operations || []).map((op: any, i: number) => ({
          id: `${log.id || 'op'}-${i}`,
          type: 'tool_call',
          timestamp: op.timestamp || parsed.timestamp_start,
          toolName,
          actionType: {
            action: mapStdoutToolAction(toolName),
            target: op.target,
            path: op.target,
            file_path: op.target,
          },
          status: 'success',
        })),
        count: parsed.total_count || parsed.operations?.length || 0,
        timestamp_start: parsed.timestamp_start,
        timestamp_end: parsed.timestamp_end,
        status: 'success',
      }];
    }

    if (tag === 'SubagentSpawn') {
      return [{
        id: log.id || `subagent-${index}`,
        type: 'subagent',
        timestamp: ts(log, parsed.timestamp),
        thread: {
          id: parsed.child_attempt_id,
          parentAttemptId: (log as { attempt_id?: string }).attempt_id,
          agentName: 'Task',
          taskDescription: parsed.task_description,
          status: 'running',
          depth: 1,
          entries: [],
          startedAt: parsed.timestamp || log.timestamp,
        },
      }];
    }

    if (tag === 'ToolStatus') {
      const toolName = parsed.tool_name || log.tool_name;
      const status = parsed.status === 'Success' ? 'success' : parsed.status === 'Failed' ? 'failed' : 'success';
      return [{
        id: log.id || `status-${index}`,
        type: 'tool_call',
        timestamp: ts(log, parsed.timestamp),
        toolName,
        actionType: { action: mapStdoutToolAction(toolName) },
        status,
        error: parsed.error_message,
      }];
    }

    if (tag === 'FileChange') {
      // Backend sends change_type as {"type":"Created"} | {"type":"Modified"} | {"type":"Renamed","from":"..."}
      const ct = parsed.change_type;
      const changeType =
        typeof ct === 'string'
          ? ct
          : ct?.type === 'Created'
            ? 'Created'
            : ct?.type === 'Deleted'
              ? 'Deleted'
              : ct?.type === 'Renamed' && typeof ct?.from === 'string'
                ? ({ Renamed: { from: ct.from } } as const)
                : 'Modified';
      return [{
        id: log.id || `file-${index}`,
        type: 'file_change',
        timestamp: ts(log, parsed.timestamp),
        path: parsed.path || '',
        changeType,
        linesAdded: parsed.lines_added,
        linesRemoved: parsed.lines_removed,
      }];
    }

    if (tag === 'TodoItem') {
      return [{
        id: log.id || `todo-${index}`,
        type: 'assistant_message',
        timestamp: ts(log, parsed.timestamp),
        content: `[${parsed.status}] ${parsed.content}`,
        source: 'normalized',
      }];
    }

    const fallbackText = typeof parsed.content === 'string' && parsed.content.trim()
      ? parsed.content
      : `Agent event: ${tag || 'normalized'}`;
    return [{
      id: log.id || `norm-${index}`,
      type: 'assistant_message',
      timestamp: ts(log, parsed.timestamp),
      content: fallbackText,
      source: 'normalized',
    }];
  } catch {
    return [];
  }
}

/** 2. Simple types - user, stderr, system */
function handleSimple(log: AgentLogLike, index: number): TimelineEntry[] | null {
  const lt = (log.log_type || '').toLowerCase();
  const content = String(log.content ?? log.message ?? '').trim();

  // Show system logs (e.g. "Starting from-scratch init", "Spawning OpenAI Codex agent")
  if (lt === 'system') {
    if (!content) return null;
    const formattedBreakdown = formatBreakdownTaskContent(content);
    return [{
      id: log.id || `system-${index}`,
      type: 'assistant_message',
      timestamp: ts(log),
      content: formattedBreakdown ?? content,
      source: 'system',
    }];
  }

  if (lt === 'user' || lt === 'stdin') {
    return [{
      id: log.id || `user-${index}`,
      type: 'user_message',
      timestamp: ts(log),
      content,
    }];
  }

  // R2: process_stderr from backend. Timeline = agent conversation only.
  // Show real errors (ERROR/WARN); hide internal logs (DEBUG/INFO/TRACE) and build noise.
  if (lt === 'stderr' || lt === 'process_stderr') {
    if (isInternalRuntimeLog(content)) return [];
    if (isBuildOutputNoise(content)) return [];
    const level = /^\S+\s+(ERROR|WARN|WARNING)\b/i.exec(content)?.[1]?.toUpperCase();
    if (level === 'ERROR' || level === 'WARN' || level === 'WARNING') {
      return [{
        id: log.id || `error-${index}`,
        type: 'error',
        timestamp: ts(log),
        error: extractErrorMessage(content),
        tool: log.tool_name,
      }];
    }
    return [];
  }

  return null;
}

/** 3. Stdout - minimal fallback (R2: prefer normalized; this for legacy/process_stdout from buffer) */
function handleStdout(log: AgentLogLike, index: number): TimelineEntry[] | null {
  const lt = (log.log_type || '').toLowerCase();
  if (lt !== 'stdout' && lt !== 'process_stdout') return null;

  const content = log.content || '';
  const formattedBreakdown = formatBreakdownTaskContent(content);
  if (formattedBreakdown) {
    return [{
      id: log.id || `assistant-${index}`,
      type: 'assistant_message',
      timestamp: ts(log),
      content: formattedBreakdown,
      source: 'stdout',
    }];
  }

  if (isNoisyTelemetry(content)) return [];
  if (isInternalRuntimeLog(content)) return [];
  if (isBuildOutputNoise(content)) return [];

  if (import.meta.env.VITE_TIMELINE_STDOUT_FALLBACK === 'false') return [];

  const codex = parseCodexJsonStdoutEntries(content, log, index);
  if (codex !== null) return codex;

  const transcript = parseStdoutTranscriptEntries(content, log, index);
  if (transcript !== null) return transcript;

  if (/^\s*(thinking:|\[thinking\])/i.test(content)) {
    return [{
      id: log.id || `thinking-${index}`,
      type: 'thinking',
      timestamp: ts(log),
      content,
    }];
  }

  if (content.match(/^(Read|Edit|Bash|Grep|Glob|Write|Task|TodoWrite|TodoRead)/i)) {
    const toolName = content.split(/\s+/)[0];
    if (/^(Read|Edit|Write)$/i.test(toolName) && containsHiddenAcpmsContractPath(content)) {
      return [];
    }
    return [{
      id: log.id || `tool-${index}`,
      type: 'tool_call',
      timestamp: ts(log),
      toolName,
      actionType: { action: mapStdoutToolAction(toolName), target: content },
      status: 'success',
    }];
  }

  return [{
    id: log.id || `assistant-${index}`,
    type: 'assistant_message',
    timestamp: ts(log),
    content,
    source: 'stdout',
  }];
}

/** 4. Legacy tool_call/action, file_change */
function handleLegacy(log: AgentLogLike, index: number): TimelineEntry[] | null {
  const lt = (log.log_type || '').toLowerCase();
  const content = log.content || '';

  if (lt === 'tool_call' || lt === 'action') {
    let actionType: any = {};
    try {
      const parsed = JSON.parse(content);
      actionType = parsed.action_type || parsed;
    } catch {
      actionType = { action: 'unknown', target: content };
    }
    const toolName = log.tool_name || actionType.tool_name || 'Unknown';
    const action = inferToolAction(toolName, actionType);
    const target = inferToolTarget(action, actionType, toolName);
    const path = actionType.file_path || actionType.path || target;
    if (isHiddenInternalFileAction(action, typeof path === 'string' ? path : undefined)) {
      return [];
    }
    const ns = normalizeToolStatus(typeof log.status === 'object' ? log.status : { status: log.status });
    return [{
      id: log.id || `tool-${index}`,
      type: 'tool_call',
      timestamp: ts(log),
      toolName,
      actionType: {
        action: action || actionType.action || 'unknown',
        file_path: actionType.file_path || actionType.path,
        path: actionType.path,
        target: target ?? undefined,
        command: actionType.command,
        query: actionType.query,
        url: actionType.url,
        todos: actionType.todos,
        operation: actionType.operation,
        description: actionType.description,
        plan: actionType.plan,
        arguments: actionType.arguments,
        result: actionType.result,
      },
      status: ns.status,
      statusReason: ns.reason,
      approvalId: ns.approvalId,
      duration: log.duration,
    }];
  }

  if (lt === 'file_change') {
    try {
      const parsed = JSON.parse(content);
      if (isHiddenAcpmsContractPath(parsed.path)) {
        return [];
      }
      return [{
        id: log.id || `file-${index}`,
        type: 'file_change',
        timestamp: ts(log),
        path: parsed.path,
        changeType: parsed.change_type,
        linesAdded: parsed.lines_added,
        linesRemoved: parsed.lines_removed,
      }];
    } catch {
      return [];
    }
  }

  return null;
}

/**
 * Parse raw log into timeline entries.
 * Order: normalized → simple → stdout → legacy → fallback (unknown types).
 */
export function normalizeLogToEntry(log: AgentLogLike, index: number): TimelineEntry[] {
  const result =
    handleNormalized(log, index) ??
    handleSimple(log, index) ??
    handleStdout(log, index) ??
    handleLegacy(log, index) ??
    handleFallback(log, index);
  return result ?? [];
}

/** Fallback for unhandled log types - show as assistant_message so logs are not dropped. */
function handleFallback(log: AgentLogLike, index: number): TimelineEntry[] | null {
  const content = String(log.content ?? log.message ?? '').trim();
  if (!content) return null;
  const formattedBreakdown = formatBreakdownTaskContent(content);
  if (!formattedBreakdown && (isNoisyTelemetry(content) || isInternalRuntimeLog(content) || isBuildOutputNoise(content))) {
    return null;
  }
  return [{
    id: log.id || `fallback-${index}`,
    type: 'assistant_message',
    timestamp: ts(log),
    content: formattedBreakdown ?? content,
    source: 'system',
  }];
}
