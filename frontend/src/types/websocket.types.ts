import { JSONPatchOperation } from '@/utils/json-patch';

export type ConnectionStatus =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'disconnected'
  | 'error';

export type ConnectionError =
  | 'network_error'
  | 'timeout'
  | 'server_error'
  | 'auth_failed'
  | 'unknown';

export interface WebSocketMessage {
  type: 'patch' | 'ping' | 'pong' | 'error' | 'complete';
  data?: {
    patches: JSONPatchOperation[];
    timestamp: string;
  };
  error?: string;
}

export type MessageCallback = (message: WebSocketMessage) => void;
export type StatusCallback = (status: ConnectionStatus) => void;
export type ErrorCallback = (error: ConnectionError, message?: string) => void;
