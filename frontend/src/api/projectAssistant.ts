import { apiGet, apiPost, API_PREFIX, getApiBaseUrl, getWsBaseUrl } from './client';

export function getAssistantLogsWsUrl(projectId: string, sessionId: string): string {
  const wsBase = getApiBaseUrl()
    ? getApiBaseUrl().replace(/^http/, 'ws')
    : getWsBaseUrl();
  return `${wsBase}${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}/logs/ws`;
}

export interface AssistantSession {
  id: string;
  project_id: string;
  user_id: string;
  status: string;
  s3_log_key: string | null;
  created_at: string;
  ended_at: string | null;
}

export interface AssistantMessage {
  id: string;
  session_id: string;
  role: string;
  content: string;
  metadata?: Record<string, unknown>;
  created_at: string;
}

export interface SessionWithMessages {
  session: AssistantSession;
  messages: AssistantMessage[];
}

export async function createSession(
  projectId: string,
  forceNew = true
): Promise<AssistantSession> {
  return apiPost<AssistantSession>(`${API_PREFIX}/projects/${projectId}/assistant/sessions`, {
    force_new: forceNew,
  });
}

export async function listSessions(projectId: string): Promise<AssistantSession[]> {
  return apiGet<AssistantSession[]>(`${API_PREFIX}/projects/${projectId}/assistant/sessions`);
}

export async function getSession(
  projectId: string,
  sessionId: string
): Promise<SessionWithMessages> {
  return apiGet<SessionWithMessages>(
    `${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}`
  );
}

export async function getSessionStatus(
  projectId: string,
  sessionId: string
): Promise<{ active: boolean }> {
  const res = await apiGet<{ active: boolean }>(
    `${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}/status`
  );
  return res;
}

export async function startSession(
  projectId: string,
  sessionId: string
): Promise<void> {
  return apiPost<void>(
    `${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}/start`,
    {}
  );
}

export interface AttachmentRef {
  key: string;
  filename?: string;
}

export async function getAssistantAttachmentUploadUrl(
  projectId: string,
  filename: string,
  contentType: string
): Promise<{ upload_url: string; key: string }> {
  return apiPost<{ upload_url: string; key: string }>(
    `${API_PREFIX}/projects/${projectId}/assistant/attachments/upload-url`,
    { filename, content_type: contentType }
  );
}

export async function postMessage(
  projectId: string,
  sessionId: string,
  content: string,
  attachments?: AttachmentRef[]
): Promise<void> {
  return apiPost<void>(
    `${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}/messages`,
    { content, attachments: attachments ?? [] }
  );
}

export async function postInput(
  projectId: string,
  sessionId: string,
  content: string
): Promise<void> {
  return apiPost<void>(
    `${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}/input`,
    { content }
  );
}

export interface ToolCall {
  id: string;
  name: string;
  args: Record<string, unknown>;
}

export interface ConfirmToolResult {
  tool_call_id: string;
  confirmed: boolean;
  entity_type?: string;
  entity_id?: string;
}

export async function confirmTool(
  projectId: string,
  sessionId: string,
  toolCallId: string,
  confirmed: boolean
): Promise<ConfirmToolResult> {
  return apiPost<ConfirmToolResult>(
    `${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}/confirm-tool`,
    { tool_call_id: toolCallId, confirmed }
  );
}

export async function endSession(
  projectId: string,
  sessionId: string
): Promise<AssistantSession> {
  return apiPost<AssistantSession>(
    `${API_PREFIX}/projects/${projectId}/assistant/sessions/${sessionId}/end`,
    {}
  );
}
