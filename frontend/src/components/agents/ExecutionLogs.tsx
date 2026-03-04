import { useEffect, useRef, useState } from 'react';
import { AgentLog, getAttemptLogs } from '../../api/taskAttempts';
import { getAccessToken } from '../../api/client';
import { ApprovalModal } from '../modals/ApprovalModal';
import type { ToolApproval } from '../../api/approvals';
import { parseStructuredLog, formatStructuredLog } from '../../utils/parseStructuredLog';
import { logger } from '@/lib/logger';

interface ExecutionLogsProps {
  attemptId: string;
}

const WS_AUTH_PROTOCOL = 'acpms-bearer';

export function ExecutionLogs({ attemptId }: ExecutionLogsProps) {
  const [logs, setLogs] = useState<AgentLog[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [pendingApprovals, setPendingApprovals] = useState<ToolApproval[]>([]);
  const [selectedApproval, setSelectedApproval] = useState<ToolApproval | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    // Initial load
    loadLogs();

    // Connect WebSocket (note: WebSocket routes are under /ws, not /api/v1)
    const wsBase = import.meta.env.VITE_WS_URL || 'ws://localhost:3000';
    const wsUrl = `${wsBase}/ws/attempts/${attemptId}/logs`;
    const token = getAccessToken();

    const ws = token
      ? new WebSocket(wsUrl, [WS_AUTH_PROTOCOL, token])
      : new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      logger.log('Connected to log stream');
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);

        if (data.type === 'Log') {
          const newLog: AgentLog = {
            id: crypto.randomUUID(),
            attempt_id: data.attempt_id,
            log_type: data.log_type,
            content: data.content,
            created_at: data.timestamp,
          };
          setLogs(prev => [...prev, newLog]);
        } else if (data.type === 'ApprovalRequest') {
          // SDK mode approval request
          const approval: ToolApproval = {
            id: data.approval_id ?? data.tool_use_id,
            attempt_id: data.attempt_id,
            execution_process_id: data.execution_process_id ?? null,
            tool_use_id: data.tool_use_id,
            tool_name: data.tool_name,
            tool_input: data.tool_input,
            status: 'pending',
            created_at: data.timestamp,
          };
          setPendingApprovals(prev => [...prev, approval]);
          setSelectedApproval(current => current ?? approval);

          // Auto-show modal for first approval
          // Add log entry for visibility
          const logEntry: AgentLog = {
            id: `approval-${data.tool_use_id}`,
            attempt_id: data.attempt_id,
            log_type: 'system',
            content: `🔐 Tool permission requested: ${data.tool_name}`,
            created_at: data.timestamp,
          };
          setLogs(prev => [...prev, logEntry]);
        }
      } catch (err) {
        logger.error('Failed to parse WS message', err);
      }
    };

    ws.onerror = (error) => {
      logger.error('WebSocket error:', error);
      // Fallback to polling could be implemented here
    };

    return () => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.close();
      }
    };
  }, [attemptId]);

  useEffect(() => {
    // Auto-scroll to bottom when new logs arrive
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const loadLogs = async () => {
    try {
      const existingLogs = await getAttemptLogs(attemptId);
      setLogs(existingLogs);
      setLoading(false);
    } catch (err) {
      setError('Failed to load logs');
      setLoading(false);
    }
  };

  const getLogColor = (logType: string) => {
    switch (logType) {
      case 'stderr':
        return 'text-red-400';
      case 'system':
        return 'text-blue-400';
      case 'input':
        return 'text-green-400';
      default:
        return 'text-gray-300';
    }
  };

  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    });
  };

  if (loading && logs.length === 0) {
    return (
      <div className="flex justify-center items-center p-8">
        <div className="text-gray-500">Loading logs...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
        {error}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="bg-gray-900 rounded-lg p-4 font-mono text-sm h-96 overflow-y-auto">
        {logs.length === 0 ? (
          <div className="text-gray-500 text-center py-8">
            No logs yet. Execution will start soon...
          </div>
        ) : (
          <div className="space-y-1">
            {logs.map((log) => {
              // Parse JSON structured logs (SDK mode)
              const parsed = parseStructuredLog(log.content);
              const displayContent = formatStructuredLog(parsed);

              return (
                <div key={log.id} className="flex gap-3">
                  <span className="text-gray-500 text-xs flex-shrink-0 select-none">
                    {formatTimestamp(log.created_at)}
                  </span>
                  <span className={`flex-1 whitespace-pre-wrap ${getLogColor(log.log_type)}`}>
                    {displayContent}
                  </span>
                </div>
              );
            })}
            <div ref={logsEndRef} />
          </div>
        )}
      </div>

      {/* Pending Approvals Badge */}
      {pendingApprovals.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {pendingApprovals.map((approval) => (
            <button
              key={approval.tool_use_id}
              onClick={() => setSelectedApproval(approval)}
              className="px-3 py-1.5 bg-orange-100 dark:bg-orange-500/20 border border-orange-300 dark:border-orange-500/50 rounded-lg text-xs font-semibold text-orange-700 dark:text-orange-300 hover:bg-orange-200 dark:hover:bg-orange-500/30 transition-colors flex items-center gap-2"
            >
              <span className="material-symbols-outlined text-sm">security</span>
              {approval.tool_name} - Pending Approval
            </button>
          ))}
        </div>
      )}

      {/* Approval Modal (SDK mode) */}
      {selectedApproval && (
        <ApprovalModal
          approval={selectedApproval}
          onClose={() => setSelectedApproval(null)}
          onResponded={(toolUseId) => {
            setPendingApprovals(prev => prev.filter(a => a.tool_use_id !== toolUseId));
            setSelectedApproval(null);
          }}
        />
      )}
    </div>
  );
}
