/**
 * LogStreamDisplay - Scrollable log entries list with auto-scroll
 * Main container for rendering the agent terminal log stream
 */

import { memo, useRef, useEffect, useState, useCallback } from 'react';
import { LogEntry } from './LogEntry';
import type { AgentLogEntry } from './types';

interface LogStreamDisplayProps {
  logs: AgentLogEntry[];
  loading?: boolean;
  isStreaming?: boolean;
  onFileClick?: (filepath: string) => void;
  className?: string;
}

export const LogStreamDisplay = memo(function LogStreamDisplay({
  logs,
  loading = false,
  isStreaming = false,
  onFileClick,
  className = '',
}: LogStreamDisplayProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const [userScrolled, setUserScrolled] = useState(false);

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (autoScroll && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  // Detect user scroll to disable auto-scroll
  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;

    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

    if (!isAtBottom && !userScrolled) {
      setUserScrolled(true);
      setAutoScroll(false);
    } else if (isAtBottom && userScrolled) {
      setUserScrolled(false);
      setAutoScroll(true);
    }
  }, [userScrolled]);

  // Scroll to bottom button handler
  const scrollToBottom = useCallback(() => {
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    setAutoScroll(true);
    setUserScrolled(false);
  }, []);

  // Empty state
  if (!loading && logs.length === 0) {
    return (
      <div className={`flex flex-col items-center justify-center py-12 ${className}`}>
        <span className="material-symbols-outlined text-4xl text-slate-600 mb-3">
          terminal
        </span>
        <p className="text-sm text-slate-500">
          {isStreaming ? 'Waiting for agent activity...' : 'No logs available'}
        </p>
        {isStreaming && (
          <div className="flex items-center gap-2 mt-3 text-xs text-slate-500">
            <span className="size-2 bg-green-500 rounded-full animate-pulse" />
            Agent session active
          </div>
        )}
      </div>
    );
  }

  return (
    <div className={`relative ${className}`}>
      {/* Logs container */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="h-full overflow-y-auto p-4 space-y-1 scroll-smooth"
      >
        {/* Loading state */}
        {loading && (
          <div className="flex items-center justify-center py-6">
            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary" />
            <span className="ml-3 text-sm text-slate-500">Loading logs...</span>
          </div>
        )}

        {/* Log entries */}
        {logs.map((log) => (
          <LogEntry key={log.id} entry={log} onFileClick={onFileClick} />
        ))}

        {/* Streaming indicator */}
        {isStreaming && logs.length > 0 && (
          <div className="flex items-center gap-2 py-2 text-xs text-slate-500">
            <span className="size-2 bg-green-500 rounded-full animate-pulse" />
            Agent is working...
          </div>
        )}

        {/* Scroll anchor */}
        <div ref={logsEndRef} />
      </div>

      {/* Scroll to bottom button */}
      {!autoScroll && logs.length > 0 && (
        <button
          onClick={scrollToBottom}
          className="absolute bottom-4 right-4 flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 border border-slate-600 rounded-full text-xs text-slate-300 shadow-lg transition-colors"
        >
          <span className="material-symbols-outlined text-[14px]">
            keyboard_arrow_down
          </span>
          New logs
        </button>
      )}

      {/* Auto-scroll indicator */}
      {autoScroll && isStreaming && (
        <div className="absolute bottom-4 right-4 flex items-center gap-1.5 px-2 py-1 bg-slate-800/80 rounded text-xs text-slate-400">
          <span className="material-symbols-outlined text-[12px]">
            vertical_align_bottom
          </span>
          Auto-scroll
        </div>
      )}
    </div>
  );
});
