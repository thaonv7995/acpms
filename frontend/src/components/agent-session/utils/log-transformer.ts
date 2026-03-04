/**
 * log-transformer - Transforms backend logs to frontend format
 */

import type { AgentLogEntry, LogEntryType } from '../types';
import { mapLogType, detectLogType, extractFileInfo } from '../types';

export interface BackendLog {
  id: string;
  attempt_id: string;
  // API uses 'type' but we also support 'log_type' for compatibility
  type?: string;
  log_type?: string;
  // API uses 'message' but we also support 'content' for compatibility
  message?: string;
  content?: string;
  // API uses 'timestamp' but we also support 'created_at' for compatibility
  timestamp?: string;
  created_at?: string;
  level?: string;
  metadata?: Record<string, unknown>;
}

export function transformLog(backendLog: BackendLog): AgentLogEntry {
  // Handle field name variations from API
  const logType = backendLog.log_type || backendLog.type || 'system';
  const content = backendLog.content || backendLog.message || '';
  const timestamp = backendLog.created_at || backendLog.timestamp || new Date().toISOString();

  let type: LogEntryType = mapLogType(logType);

  // Try to detect more specific type from content
  type = detectLogType(content, type);

  // Extract file info if applicable
  let metadata = backendLog.metadata || {};
  if (type === 'file_read' || type === 'file_write') {
    const fileInfo = extractFileInfo(content);
    if (fileInfo) {
      metadata = { ...metadata, ...fileInfo };
    }
  }

  return {
    id: backendLog.id,
    type,
    content,
    timestamp,
    metadata: metadata as AgentLogEntry['metadata'],
  };
}

export function transformLogs(backendLogs: BackendLog[]): AgentLogEntry[] {
  return backendLogs.map(transformLog);
}
