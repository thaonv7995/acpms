import { useState, useEffect, useRef } from 'react';
import { useAttemptStream } from '../../hooks/useAttemptStream';

interface AgentLogsSectionProps {
  attemptId: string;
  status: string;
}

export function AgentLogsSection({ attemptId, status }: AgentLogsSectionProps) {
  const [autoScroll, setAutoScroll] = useState(true);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const isRunning = status.toLowerCase() === 'running';

  // Use SSE streaming hook
  const { logs, isConnected, isLoading, error, reconnect } = useAttemptStream(attemptId);

  useEffect(() => {
    if (autoScroll && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  const getLogTypeColor = (logType: string) => {
    switch (logType.toLowerCase()) {
      case 'stderr':
      case 'error':
        return 'text-red-400';
      case 'warning':
        return 'text-yellow-400';
      case 'success':
        return 'text-green-400';
      case 'tool':
        return 'text-blue-400';
      case 'thinking':
        return 'text-purple-400';
      case 'system':
        return 'text-cyan-400';
      default:
        return 'text-slate-300';
    }
  };

  const getLogTypeIcon = (logType: string) => {
    switch (logType.toLowerCase()) {
      case 'stderr':
      case 'error':
        return 'error';
      case 'warning':
        return 'warning';
      case 'success':
        return 'check_circle';
      case 'tool':
        return 'build';
      case 'thinking':
        return 'psychology';
      case 'system':
        return 'settings';
      default:
        return 'terminal';
    }
  };

  return (
    <div className="bg-white dark:bg-surface-dark rounded-xl border border-slate-200 dark:border-slate-700 overflow-hidden">
      {/* Header */}
      <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-700 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="material-symbols-outlined text-[18px] text-slate-500">terminal</span>
          <h3 className="text-sm font-bold text-slate-900 dark:text-white uppercase">
            Agent Logs
          </h3>
          {isConnected && (
            <span className="flex items-center gap-1.5 px-2 py-0.5 bg-green-500/10 text-green-500 text-xs font-medium rounded-full">
              <span className="size-1.5 bg-green-500 rounded-full animate-pulse"></span>
              Streaming
            </span>
          )}
          {!isConnected && isRunning && (
            <span className="flex items-center gap-1.5 px-2 py-0.5 bg-yellow-500/10 text-yellow-500 text-xs font-medium rounded-full">
              <span className="size-1.5 bg-yellow-500 rounded-full"></span>
              Reconnecting...
            </span>
          )}
          {error && (
            <span className="flex items-center gap-1.5 px-2 py-0.5 bg-red-500/10 text-red-500 text-xs font-medium rounded-full">
              <span className="material-symbols-outlined text-[12px]">error</span>
              {error}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setAutoScroll(!autoScroll)}
            className={`p-1.5 rounded-lg transition-colors ${
              autoScroll
                ? 'bg-primary/10 text-primary'
                : 'text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
            title={autoScroll ? 'Auto-scroll on' : 'Auto-scroll off'}
          >
            <span className="material-symbols-outlined text-[18px]">
              {autoScroll ? 'vertical_align_bottom' : 'pause'}
            </span>
          </button>
          <button
            onClick={reconnect}
            className="p-1.5 text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors"
            title="Reconnect stream"
          >
            <span className="material-symbols-outlined text-[18px]">refresh</span>
          </button>
        </div>
      </div>

      {/* Logs content */}
      <div className="bg-[#0d1117] max-h-[400px] overflow-y-auto font-mono text-sm">
        {isLoading ? (
          <div className="p-6 text-center text-slate-500">
            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary mx-auto mb-2"></div>
            Connecting to stream...
          </div>
        ) : logs.length === 0 ? (
          <div className="p-6 text-center text-slate-500">
            <span className="material-symbols-outlined text-3xl mb-2 block">hourglass_empty</span>
            {isRunning ? 'Waiting for logs...' : 'No logs available'}
          </div>
        ) : (
          <div className="p-4 space-y-1">
            {logs.map((log, index) => (
              <div
                key={log.id || `${log.attempt_id}-${index}`}
                className="flex gap-2 group hover:bg-slate-800/50 px-2 py-1 rounded"
              >
                <span className={`material-symbols-outlined text-[14px] mt-0.5 ${getLogTypeColor(log.log_type)}`}>
                  {getLogTypeIcon(log.log_type)}
                </span>
                <span className="text-slate-500 text-xs whitespace-nowrap">
                  {log.timestamp || log.created_at
                    ? new Date(log.timestamp || log.created_at!).toLocaleTimeString()
                    : ''}
                </span>
                <span className={`flex-1 whitespace-pre-wrap break-all ${getLogTypeColor(log.log_type)}`}>
                  {log.content}
                </span>
              </div>
            ))}
            <div ref={logsEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
