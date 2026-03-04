/**
 * AgentSessionPanel - Right panel showing agent execution session
 *
 * Features:
 * - Real-time log streaming via WebSocket
 * - File change display with diff links
 * - Summary & Actions card
 * - Chat input with @mentions
 * - Navigation to Diff Viewer
 */

import { useCallback, useMemo, useState } from 'react';
import type { KanbanTask } from '../../types/project';
import { LogStreamDisplay } from './LogStreamDisplay';
import { SummaryActions } from './SummaryActions';
import { ChatInput } from './ChatInput';
import { PanelHeader } from './PanelHeader';
import { TerminalHeader } from './TerminalHeader';
import { useAgentSession } from './useAgentSession';

interface AgentSessionPanelProps {
  task: KanbanTask;
  taskId: string;
  attemptId?: string;
  projectId?: string;
  onClose: () => void;
  onViewDiff?: (attemptId: string) => void;
}

export function AgentSessionPanel({
  task,
  taskId,
  attemptId,
  projectId,
  onClose,
  onViewDiff,
}: AgentSessionPanelProps) {
  const [isSending, setIsSending] = useState(false);

  // Generate branch name from task
  const branchName = useMemo(() => {
    const slug = task.title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .slice(0, 30);
    return `vk/${taskId.slice(0, 8)}-${slug}`;
  }, [task.title, taskId]);

  // Agent session hook
  const { state, isConnected, isLoading, error, sendMessage, refresh } = useAgentSession({
    attemptId,
    enabled: !!attemptId,
  });

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    },
    [onClose]
  );

  const handleSendMessage = useCallback(
    async (message: string) => {
      setIsSending(true);
      try {
        await sendMessage(message);
      } finally {
        setIsSending(false);
      }
    },
    [sendMessage]
  );

  const handleViewChanges = useCallback(() => {
    if (attemptId && onViewDiff) {
      onViewDiff(attemptId);
    }
  }, [attemptId, onViewDiff]);

  const handleFileClick = useCallback(
    (_filepath: string) => {
      if (attemptId && onViewDiff) {
        onViewDiff(attemptId);
      }
    },
    [attemptId, onViewDiff]
  );

  const handleCopyOutput = useCallback(() => {
    const logText = state.logs.map((log) => log.content).join('\n');
    navigator.clipboard.writeText(logText);
  }, [state.logs]);

  // Diff summary with defaults
  const diffSummary = state.diffSummary || { filesChanged: 0, additions: 0, deletions: 0 };
  const isRunning = state.status === 'running';
  const canSendMessage = isConnected && (isRunning || state.status === 'waiting_input');

  return (
    <div
      className="h-full flex flex-col bg-white dark:bg-slate-900"
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      <PanelHeader
        task={task}
        branchName={state.branch || branchName}
        isConnected={isConnected}
        status={state.status}
        onClose={onClose}
        onRefresh={refresh}
      />

      <div className="flex-1 flex flex-col min-h-0 bg-terminal-bg dark:bg-[#0d1117]">
        <TerminalHeader isRunning={isRunning} />

        {error && (
          <div className="mx-4 mt-2 px-3 py-2 bg-red-500/10 border border-red-500/30 rounded text-sm text-red-400">
            {error}
          </div>
        )}

        <LogStreamDisplay
          logs={state.logs}
          loading={isLoading}
          isStreaming={isRunning}
          onFileClick={handleFileClick}
          className="flex-1 min-h-0"
        />

        {(state.status === 'completed' || state.status === 'failed' || diffSummary.filesChanged > 0) && (
          <div className="px-4 pb-4">
            <SummaryActions
              summary={diffSummary}
              status={state.status}
              onViewChanges={handleViewChanges}
              onCopyOutput={handleCopyOutput}
              onRestart={() => {}}
              onCancel={() => {}}
            />
          </div>
        )}
      </div>

      <div className="border-t border-slate-200 dark:border-slate-700 p-4 bg-slate-50 dark:bg-slate-800/50">
        <ChatInput
          onSend={handleSendMessage}
          disabled={!canSendMessage}
          isLoading={isSending}
          projectId={projectId}
          placeholder={
            !attemptId
              ? 'No active agent session'
              : !isConnected
                ? 'Connecting to agent...'
                : 'Continue working... Type @ to search files'
          }
        />
      </div>
    </div>
  );
}
