/**
 * session-api - REST API calls for agent session
 */

import type { AgentLogEntry, AgentSessionState } from '../types';
import { transformLogs, type BackendLog } from './log-transformer';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3000';

function getAuthHeaders(): HeadersInit {
  const token = localStorage.getItem('acpms_token');
  return {
    Authorization: `Bearer ${token}`,
    'Content-Type': 'application/json',
  };
}

export async function fetchAttemptLogs(attemptId: string): Promise<AgentLogEntry[]> {
  const response = await fetch(`${API_BASE}/api/v1/attempts/${attemptId}/logs`, {
    headers: getAuthHeaders(),
  });

  if (!response.ok) {
    throw new Error('Failed to fetch logs');
  }

  const json = await response.json();
  // API returns wrapped response: { success, data: [...] }
  const logs: BackendLog[] = json.data || json || [];
  return transformLogs(logs);
}

export interface AttemptStatus {
  id: string;
  status: string;
  branch?: string;
  error_message?: string;
}

export async function fetchAttemptStatus(attemptId: string): Promise<AttemptStatus> {
  const response = await fetch(`${API_BASE}/api/v1/attempts/${attemptId}`, {
    headers: getAuthHeaders(),
  });

  if (!response.ok) {
    throw new Error('Failed to fetch attempt status');
  }

  return response.json();
}

export async function sendAttemptInput(attemptId: string, input: string): Promise<void> {
  const response = await fetch(`${API_BASE}/api/v1/attempts/${attemptId}/input`, {
    method: 'POST',
    headers: getAuthHeaders(),
    body: JSON.stringify({ input }),
  });

  if (!response.ok) {
    throw new Error('Failed to send input');
  }
}

export function mapStatusToState(backendStatus: string): AgentSessionState['status'] {
  const statusMap: Record<string, AgentSessionState['status']> = {
    QUEUED: 'idle',
    RUNNING: 'running',
    SUCCESS: 'completed',
    FAILED: 'failed',
    CANCELLED: 'cancelled',
    WAITING_INPUT: 'waiting_input',
  };
  return statusMap[backendStatus] || 'idle';
}
