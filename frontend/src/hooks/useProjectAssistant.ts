import { useState, useCallback, useEffect, useRef } from 'react';
import {
  createSession,
  listSessions,
  getSession,
  postMessage,
  postInput,
  endSession as apiEndSession,
  startSession as apiStartSession,
  getSessionStatus,
  getAssistantLogsWsUrl,
  type AssistantSession,
  type AssistantMessage,
  type SessionWithMessages,
  type AttachmentRef,
} from '@/api/projectAssistant';
import { getAccessToken } from '@/api/client';

export function useProjectAssistant(projectId: string | undefined) {
  const [session, setSession] = useState<AssistantSession | null>(null);
  const [messages, setMessages] = useState<AssistantMessage[]>([]);
  const [sessions, setSessions] = useState<AssistantSession[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [agentActive, setAgentActive] = useState(false);
  const [starting, setStarting] = useState(false);

  const createNewSession = useCallback(
    async (forceNew = true) => {
      if (!projectId) return null;
      setLoading(true);
      setError(null);
      try {
        const s = await createSession(projectId, forceNew);
        setSession(s);
        setAgentActive(false);
        setStarting(false);
        if (forceNew) {
          setMessages([]);
        } else if (s.id) {
          // Load messages khi reuse session
          const data = await getSession(projectId, s.id);
          setMessages(data.messages);
        }
        return s;
      } catch (e) {
        const msg = e instanceof Error ? e.message : 'Failed to create session';
        setError(msg);
        return null;
      } finally {
        setLoading(false);
      }
    },
    [projectId]
  );

  const loadSessions = useCallback(async () => {
    if (!projectId) return;
    setLoading(true);
    setError(null);
    try {
      const list = await listSessions(projectId);
      setSessions(list);
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Failed to list sessions';
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  const loadSession = useCallback(
    async (sessionId: string) => {
      if (!projectId) return;
      setLoading(true);
      setError(null);
      try {
        const data: SessionWithMessages = await getSession(projectId, sessionId);
        setSession(data.session);
        setMessages(data.messages);
      } catch (e) {
        const msg = e instanceof Error ? e.message : 'Failed to load session';
        setError(msg);
      } finally {
        setLoading(false);
      }
    },
    [projectId]
  );

  const pollIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const statusPollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  const startPolling = useCallback(() => {
    if (pollIntervalRef.current) return;
    pollIntervalRef.current = setInterval(() => {
      if (projectId && session) {
        getSession(projectId, session.id)
          .then((data) => {
            setMessages((prev) => {
              const byId = new Map(data.messages.map((m) => [m.id, m]));
              for (const m of prev) {
                if (!byId.has(m.id)) byId.set(m.id, m);
              }
              return [...byId.values()].sort(
                (a, b) =>
                  new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
              );
            });
          })
          .catch(() => {});
      }
    }, 2000);
  }, [projectId, session]);

  const refreshSession = useCallback(async () => {
    if (!projectId || !session) return;
    try {
      const data = await getSession(projectId, session.id);
      setSession(data.session);
      setMessages(data.messages);
    } catch {
      // ignore
    }
  }, [projectId, session]);

  const startAgent = useCallback(async () => {
    if (!projectId || !session) return;
    const START_REQUEST_TIMEOUT_MS = 15_000;
    const STARTING_TIMEOUT_MS = 10_000;
    const startedAt = Date.now();
    setStarting(true);
    setError(null);
    try {
      let startRequestTimeout: ReturnType<typeof setTimeout> | null = null;
      try {
        await Promise.race([
          apiStartSession(projectId, session.id),
          new Promise<never>((_, reject) => {
            startRequestTimeout = setTimeout(() => {
              reject(new Error('Start request timed out. Please retry.'));
            }, START_REQUEST_TIMEOUT_MS);
          }),
        ]);
      } finally {
        if (startRequestTimeout) {
          clearTimeout(startRequestTimeout);
        }
      }
      setAgentActive(false);
      if (statusPollRef.current) {
        clearInterval(statusPollRef.current);
        statusPollRef.current = null;
      }
      // Poll logs immediately so fast agent runs still surface assistant replies
      // even when status polling misses a short-lived active window.
      startPolling();
      void refreshSession();
      const checkActive = () => {
        getSessionStatus(projectId, session.id)
          .then((res) => {
            if (res.active) {
              setAgentActive(true);
              setStarting(false);
              refreshSession();
              if (statusPollRef.current) {
                clearInterval(statusPollRef.current);
                statusPollRef.current = null;
              }
              return;
            }
            setAgentActive(false);
            refreshSession();
            if (Date.now() - startedAt >= STARTING_TIMEOUT_MS) {
              setStarting(false);
              if (statusPollRef.current) {
                clearInterval(statusPollRef.current);
                statusPollRef.current = null;
              }
            }
          })
          .catch(() => {});
      };
      statusPollRef.current = setInterval(checkActive, 500);
      checkActive();
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Failed to start agent';
      setError(msg);
      setStarting(false);
    }
  }, [projectId, session, refreshSession, startPolling]);

  const pollAgentStatus = useCallback(() => {
    if (!projectId || !session) return;
    getSessionStatus(projectId, session.id)
      .then((res) => setAgentActive(res.active))
      .catch(() => {});
  }, [projectId, session]);

  const sendMessage = useCallback(
    async (content: string, attachments?: AttachmentRef[]) => {
      if (!projectId || !session) return false;
      setLoading(true);
      setError(null);
      try {
        try {
          await postMessage(projectId, session.id, content, attachments);
        } catch (e: unknown) {
          const err = e as { status?: number };
          if (err.status === 409) {
            if (attachments?.length) {
              setError(
                'Attachments can only be sent when the agent is idle. Wait for the current run to finish, then retry.'
              );
              return false;
            }
            try {
              await postInput(projectId, session.id, content);
            } catch {
              await postMessage(projectId, session.id, content, attachments);
            }
          } else {
            throw e;
          }
        }
        await refreshSession();
        setError(null);
        startPolling();
        return true;
      } catch (e) {
        const msg = e instanceof Error ? e.message : 'Failed to send message';
        setError(msg);
        return false;
      } finally {
        setLoading(false);
      }
    },
    [projectId, session, refreshSession, startPolling]
  );

  useEffect(() => {
    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (!session || !projectId) {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      if (statusPollRef.current) {
        clearInterval(statusPollRef.current);
        statusPollRef.current = null;
      }
      setAgentActive(false);
      return;
    }
    pollAgentStatus();
    const id = setInterval(pollAgentStatus, 3000);
    return () => clearInterval(id);
  }, [session, projectId, pollAgentStatus]);

  // Connect WS khi có session để nhận spawn logs + error real-time (kể cả khi starting)
  useEffect(() => {
    if (!projectId || !session || wsRef.current?.readyState === WebSocket.OPEN) return;
    const url = getAssistantLogsWsUrl(projectId, session.id);
    const token = getAccessToken();
    const ws = token
      ? new WebSocket(url, ['acpms-bearer', token])
      : new WebSocket(url);
    wsRef.current = ws;
    const sid = session.id;
    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg.type === 'assistant_log' && msg.session_id === sid) {
          const content = msg.content ?? '';
          if (msg.role === 'system' && content.startsWith('Error:')) {
            setStarting(false);
            if (statusPollRef.current) {
              clearInterval(statusPollRef.current);
              statusPollRef.current = null;
            }
          }
          setMessages((prev) => [
            ...prev,
            {
              id: msg.id,
              session_id: msg.session_id,
              role: msg.role ?? 'assistant',
              content,
              metadata: msg.metadata,
              created_at: msg.created_at ?? new Date().toISOString(),
            },
          ]);
        }
      } catch {
        // ignore
      }
    };
    ws.onclose = () => {
      wsRef.current = null;
    };
    return () => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [projectId, session?.id]);

  const endSession = useCallback(
    async (sessionId: string) => {
      if (!projectId) return;
      setLoading(true);
      setError(null);
      try {
        await apiEndSession(projectId, sessionId);
        if (session?.id === sessionId) {
          setSession(null);
          setMessages([]);
          setAgentActive(false);
          setStarting(false);
        }
        await loadSessions();
      } catch (e) {
        const msg = e instanceof Error ? e.message : 'Failed to end session';
        setError(msg);
      } finally {
        setLoading(false);
      }
    },
    [projectId, session?.id, loadSessions]
  );

  return {
    session,
    messages,
    sessions,
    loading,
    error,
    agentActive,
    starting,
    createSession: createNewSession,
    listSessions: loadSessions,
    loadSession,
    startAgent,
    sendMessage,
    refreshSession,
    endSession,
  };
}
