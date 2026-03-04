import { useState, useEffect, useCallback, useRef } from 'react';
import { WebSocketService } from '@/services/websocket-service';
import { ConnectionStatus, ConnectionError } from '@/types/websocket.types';

export interface UseWebSocketConnectionParams {
  url: string;
  enabled?: boolean;
  onMessage?: (message: any) => void;
}

export interface UseWebSocketConnectionReturn {
  status: ConnectionStatus;
  error: string | null;
  connect: () => void;
  disconnect: () => void;
  send: (message: any) => void;
  isConnected: boolean;
  isConnecting: boolean;
  isReconnecting: boolean;
}

export function useWebSocketConnection({
  url,
  enabled = true,
  onMessage,
}: UseWebSocketConnectionParams): UseWebSocketConnectionReturn {
  const [status, setStatus] = useState<ConnectionStatus>('idle');
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocketService | null>(null);

  const handleStatusChange = useCallback((newStatus: ConnectionStatus) => {
    setStatus(newStatus);
    if (newStatus === 'connected') {
      setError(null);
    }
  }, []);

  const handleError = useCallback((errorType: ConnectionError, message?: string) => {
    const errorMessages: Record<ConnectionError, string> = {
      network_error: 'Network error. Check your connection.',
      timeout: 'Connection timed out.',
      server_error: 'Server error. Please try again.',
      auth_failed: 'Authentication failed.',
      unknown: 'An unknown error occurred.',
    };

    setError(message || errorMessages[errorType]);
  }, []);

  const connect = useCallback(() => {
    if (!wsRef.current) {
      wsRef.current = new WebSocketService({
        url,
        onMessage,
        onStatusChange: handleStatusChange,
        onError: handleError,
      });
    }

    wsRef.current.connect();
  }, [url, onMessage, handleStatusChange, handleError]);

  const disconnect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.disconnect();
      wsRef.current = null;
    }
    setStatus('disconnected');
  }, []);

  const send = useCallback((message: any) => {
    if (wsRef.current) {
      wsRef.current.send(message);
    }
  }, []);

  // Auto-connect when enabled
  useEffect(() => {
    if (enabled) {
      connect();
    } else {
      disconnect();
    }

    return () => {
      disconnect();
    };
  }, [enabled, connect, disconnect]);

  const isConnected = status === 'connected';
  const isConnecting = status === 'connecting';
  const isReconnecting = status === 'reconnecting';

  return {
    status,
    error,
    connect,
    disconnect,
    send,
    isConnected,
    isConnecting,
    isReconnecting,
  };
}
