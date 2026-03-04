import {
  ConnectionStatus,
  ConnectionError,
  WebSocketMessage,
  MessageCallback,
  StatusCallback,
  ErrorCallback,
} from '@/types/websocket.types';
import { logger } from '@/lib/logger';

const USE_MOCK = import.meta.env.VITE_USE_MOCK_WEBSOCKET === 'true';

interface WebSocketServiceConfig {
  url: string;
  protocols?: string | string[];
  onMessage?: MessageCallback;
  onStatusChange?: StatusCallback;
  onError?: ErrorCallback;
  heartbeatInterval?: number;
  reconnectDelays?: number[];
  maxReconnectDelay?: number;
}

export class WebSocketService {
  private ws: WebSocket | null = null;
  private url: string;
  private protocols?: string | string[];
  private status: ConnectionStatus = 'idle';
  private reconnectAttempt = 0;
  private reconnectTimer: number | null = null;
  private heartbeatTimer: number | null = null;
  private messageQueue: any[] = [];

  private onMessageCallback?: MessageCallback;
  private onStatusCallback?: StatusCallback;
  private onErrorCallback?: ErrorCallback;

  private readonly heartbeatInterval: number;
  private readonly reconnectDelays: number[];
  private readonly maxReconnectDelay: number;

  constructor(config: WebSocketServiceConfig) {
    this.url = config.url;
    this.protocols = config.protocols;
    this.onMessageCallback = config.onMessage;
    this.onStatusCallback = config.onStatusChange;
    this.onErrorCallback = config.onError;
    this.heartbeatInterval = config.heartbeatInterval ?? 30000;
    this.reconnectDelays = config.reconnectDelays ?? [1000, 2000, 4000, 8000];
    this.maxReconnectDelay = config.maxReconnectDelay ?? 30000;
  }

  connect(): void {
    if (this.status === 'connected' || this.status === 'connecting') {
      return;
    }

    this.setStatus('connecting');

    if (USE_MOCK) {
      this.connectMock();
      return;
    }

    try {
      this.ws = this.protocols ? new WebSocket(this.url, this.protocols) : new WebSocket(this.url);

      this.ws.onopen = () => {
        this.handleOpen();
      };

      this.ws.onmessage = (event) => {
        this.handleMessage(event);
      };

      this.ws.onerror = () => {
        this.handleError('network_error');
      };

      this.ws.onclose = () => {
        this.handleClose();
      };
    } catch (error) {
      this.handleError('network_error', error instanceof Error ? error.message : undefined);
    }
  }

  disconnect(): void {
    this.clearTimers();
    this.setStatus('disconnected');

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  send(message: any): void {
    if (this.status !== 'connected') {
      this.messageQueue.push(message);
      return;
    }

    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }

  getStatus(): ConnectionStatus {
    return this.status;
  }

  private handleOpen(): void {
    this.reconnectAttempt = 0;
    this.setStatus('connected');
    this.startHeartbeat();
    this.flushMessageQueue();
  }

  private handleMessage(event: MessageEvent): void {
    try {
      const message: WebSocketMessage = JSON.parse(event.data);

      if (message.type === 'ping') {
        this.send({ type: 'pong' });
        return;
      }

      if (message.type === 'pong') {
        return;
      }

      this.onMessageCallback?.(message);
    } catch (error) {
      logger.error('Failed to parse WebSocket message:', error);
    }
  }

  private handleError(errorType: ConnectionError, message?: string): void {
    this.onErrorCallback?.(errorType, message);
  }

  private handleClose(): void {
    this.clearTimers();

    if (this.status === 'disconnected') {
      return;
    }

    this.setStatus('reconnecting');
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) {
      return;
    }

    const delay =
      this.reconnectAttempt < this.reconnectDelays.length
        ? this.reconnectDelays[this.reconnectAttempt]
        : this.maxReconnectDelay;

    this.reconnectAttempt++;

    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  private startHeartbeat(): void {
    this.clearHeartbeat();

    this.heartbeatTimer = window.setInterval(() => {
      this.send({ type: 'ping' });
    }, this.heartbeatInterval);
  }

  private clearHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private clearTimers(): void {
    this.clearHeartbeat();

    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private flushMessageQueue(): void {
    while (this.messageQueue.length > 0) {
      const message = this.messageQueue.shift();
      this.send(message);
    }
  }

  private setStatus(status: ConnectionStatus): void {
    if (this.status === status) return;

    this.status = status;
    this.onStatusCallback?.(status);
  }

  // Mock WebSocket for development
  private connectMock(): void {
    logger.log('[Mock WebSocket] Connecting to:', this.url);

    setTimeout(() => {
      this.handleOpen();

      // Simulate receiving messages
      const mockInterval = setInterval(() => {
        if (this.status !== 'connected') {
          clearInterval(mockInterval);
          return;
        }

        const mockMessage: WebSocketMessage = {
          type: 'patch',
          data: {
            patches: [
              {
                op: 'add',
                path: '/logs/-',
                value: {
                  id: `log-${Date.now()}`,
                  type: 'assistant_message',
                  content: `Mock log entry at ${new Date().toISOString()}`,
                  timestamp: new Date().toISOString(),
                },
              },
            ],
            timestamp: new Date().toISOString(),
          },
        };

        this.onMessageCallback?.(mockMessage);
      }, 3000);
    }, 500);
  }
}
