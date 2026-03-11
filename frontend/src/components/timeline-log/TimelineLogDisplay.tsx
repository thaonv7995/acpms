import { useEffect, useMemo, useState, useCallback } from 'react';
import { TimelineHeader } from './TimelineHeader';
import { TimelineScrollContainer } from './TimelineScrollContainer';
import { TimelineMessageBlockRenderer, groupEntriesIntoBlocks } from './TimelineMessageBlockRenderer';
import { ChatInputBar } from './ChatInputBar';
import { useTimelineStream } from '@/hooks/useTimelineStream';
import { DiffViewerModal } from '../task-detail-page/DiffViewerModal';
import { prefetchDiffData } from '../diff-viewer/useDiff';
import { updateAttemptLog } from '@/api/taskAttempts';
import type { TimelineEntry, TimelineTokenUsageInfo } from '@/types/timeline-log';
import type { ExecutionProcess } from '@/api/executionProcesses';
import { parseTodoItems, type TodoSummaryItem } from './todo-utils';
import { Check, Circle, CircleDot, ChevronUp } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { logger } from '@/lib/logger';

const TODO_PANEL_OPEN_KEY = 'timeline-todo-panel-open';

interface TimelineLogDisplayProps {
  attemptId: string | undefined;
  /** When provided (e.g. from TaskAttemptPanel), avoids duplicate execution-processes stream subscription */
  executionProcesses?: ExecutionProcess[];
  /** When set and status is running, header shows elapsed time. */
  attemptStartedAt?: string | null;
  onSendMessage?: (message: string) => Promise<void>;
  enableChat?: boolean;
  enableEditReset?: boolean;
  attemptStatus?: string;
  onAttemptStatusChange?: (status: string | null) => void;
  onMetaSnapshotChange?: (snapshot: {
    attemptStatus: string | null;
    tokenUsageInfo: TimelineTokenUsageInfo | null;
  }) => void;
  showStatusInHeader?: boolean;
  showTokenUsageInHeader?: boolean;
}

/**
 * Root timeline log display component.
 * Orchestrates header, scroll container, and optional chat input.
 */
export function TimelineLogDisplay({
  attemptId,
  executionProcesses,
  attemptStartedAt,
  onSendMessage,
  enableChat = false,
  enableEditReset = true,
  attemptStatus,
  onAttemptStatusChange,
  onMetaSnapshotChange,
  showStatusInHeader = true,
  showTokenUsageInHeader = true,
}: TimelineLogDisplayProps) {
  const [selectedDiffId, setSelectedDiffId] = useState<string | null>(null);
  const [focusFilePath, setFocusFilePath] = useState<string | undefined>(undefined);
  const [isPreparingDiffModal, setIsPreparingDiffModal] = useState(false);
  const [editingEntryId, setEditingEntryId] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState('');
  const [isSavingEdit, setIsSavingEdit] = useState(false);
  const [resetComingSoonOpen, setResetComingSoonOpen] = useState(false);

  const {
    entries,
    tokenUsageInfo,
    isLoading,
    error,
    attemptStatus: streamAttemptStatus,
    streamState,
    autoScroll,
    hasMoreOlder,
    isLoadingOlder,
    loadOlder,
    reconnect,
  } = useTimelineStream({
    attemptId,
    executionProcesses,
    enableGrouping: true,
    enableSubagentDetection: true,
    enableAutoScroll: true,
  });

  const handleViewDiff = async (diffId: string, filePath?: string) => {
    if (isPreparingDiffModal) return;
    setIsPreparingDiffModal(true);
    if (attemptId) {
      try {
        await prefetchDiffData(attemptId);
      } catch (err) {
        logger.warn('Prefetch diff before modal open failed', err);
      }
    }
    setSelectedDiffId(diffId);
    setFocusFilePath(filePath);
    setIsPreparingDiffModal(false);
  };

  const handleCloseDiff = () => {
    setSelectedDiffId(null);
    setFocusFilePath(undefined);
  };

  const handleEditUserMessage = useCallback(
    (entryId: string, content: string) => {
      setEditingEntryId(entryId);
      setEditingContent(content);
    },
    []
  );

  const handleResetUserMessage = useCallback(() => {
    setResetComingSoonOpen(true);
  }, []);

  const handleSaveEdit = useCallback(async () => {
    if (!attemptId || !editingEntryId || isSavingEdit) return;
    setIsSavingEdit(true);
    try {
      await updateAttemptLog(attemptId, editingEntryId, editingContent);
      setEditingEntryId(null);
      setEditingContent('');
      reconnect();
    } catch (err) {
      logger.error('Failed to update message:', err);
    } finally {
      setIsSavingEdit(false);
    }
  }, [attemptId, editingEntryId, editingContent, isSavingEdit, reconnect]);

  const handleCloseEditModal = useCallback(() => {
    if (!isSavingEdit) {
      setEditingEntryId(null);
      setEditingContent('');
    }
  }, [isSavingEdit]);

  const resolvedAttemptStatus = streamAttemptStatus ?? attemptStatus ?? null;
  const runtimeTodos = useMemo(() => extractLatestRuntimeTodos(entries), [entries]);

  const normalizedStatus = resolvedAttemptStatus?.toLowerCase();
  const isAttemptRunningOrQueued = normalizedStatus
    ? normalizedStatus === 'running' || normalizedStatus === 'queued'
    : false;
  const isAttemptActive = normalizedStatus
    ? normalizedStatus === 'running' || normalizedStatus === 'queued'
    : streamState === 'live' ||
    streamState === 'stale' ||
    streamState === 'reconnecting' ||
    streamState === 'connecting';

  const hasPendingApproval = useMemo(
    () =>
      entries.some(
        (entry) => entry.type === 'tool_call' && entry.status === 'pending_approval'
      ),
    [entries]
  );

  const shouldShowRuntimeTodos = isAttemptActive && runtimeTodos.length > 0;
  const visibleEntries = useMemo(
    () =>
      shouldShowRuntimeTodos
        ? entries.filter((entry) => !isTodoToolEntry(entry))
        : entries,
    [entries, shouldShowRuntimeTodos]
  );

  const entriesWithLoading = useMemo(() => {
    // Mirror Vibe Kanban:
    // - If the agent is still running, append a synthetic "loading" entry.
    // - Don't show it when we're blocked on user approval.
    if (!attemptId) return visibleEntries;
    if (!isAttemptRunningOrQueued) return visibleEntries;
    if (hasPendingApproval) return visibleEntries;

    const timestamp =
      visibleEntries.length > 0
        ? visibleEntries[visibleEntries.length - 1]?.timestamp
        : new Date().toISOString();

    let text = 'Initializing...';
    if (visibleEntries.length > 0) {
      const lastEntry = visibleEntries[visibleEntries.length - 1];
      if (lastEntry.type === 'tool_call') {
        text = `Working on ${lastEntry.toolName}...`;
      } else if (lastEntry.type === 'assistant_message') {
        text = 'Generating...';
      } else if (lastEntry.type === 'thinking') {
        text = 'Thinking...';
      } else {
        text = 'Working...';
      }
    }

    return [
      ...visibleEntries,
      {
        id: `loading-${attemptId}`,
        type: 'loading',
        timestamp,
        text,
      } as TimelineEntry & { text: string },
    ];
  }, [attemptId, hasPendingApproval, isAttemptRunningOrQueued, visibleEntries]);

  const groupedBlocks = useMemo(() => groupEntriesIntoBlocks(entriesWithLoading), [entriesWithLoading]);

  const lastQueuedUserBlockId = useMemo(() => {
    if (!isAttemptRunningOrQueued) return null;
    const lastBlock = groupedBlocks[groupedBlocks.length - 1];
    return lastBlock?.type === 'user' ? lastBlock.id : null;
  }, [groupedBlocks, isAttemptRunningOrQueued]);

  const activeStreamingId = useMemo(() => {
    if (!isAttemptRunningOrQueued) return null;
    for (let i = entriesWithLoading.length - 1; i >= 0; i--) {
      if (entriesWithLoading[i].type === 'assistant_message') {
        return entriesWithLoading[i].id;
      }
    }
    return null;
  }, [entriesWithLoading, isAttemptRunningOrQueued]);

  useEffect(() => {
    if (resolvedAttemptStatus !== undefined) {
      onAttemptStatusChange?.(resolvedAttemptStatus);
    }
  }, [resolvedAttemptStatus]);

  useEffect(() => {
    if (resolvedAttemptStatus !== undefined || tokenUsageInfo !== undefined) {
      onMetaSnapshotChange?.({
        attemptStatus: resolvedAttemptStatus,
        tokenUsageInfo: tokenUsageInfo ?? null,
      });
    }
  }, [resolvedAttemptStatus, tokenUsageInfo]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <div className="w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin mx-auto mb-4" />
          <p className="text-sm text-muted-foreground">Loading timeline...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center max-w-md">
          <div className="text-destructive mb-2">
            <svg
              className="w-12 h-12 mx-auto"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          </div>
          <h3 className="text-lg font-semibold text-foreground mb-1">Connection Error</h3>
          <p className="text-sm text-muted-foreground">{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-background">
      {/* Header */}
      {(showStatusInHeader || showTokenUsageInHeader) && (
        <TimelineHeader
          streamState={streamState}
          attemptStatus={resolvedAttemptStatus ?? undefined}
          attemptStartedAt={attemptStartedAt}
          tokenUsageInfo={tokenUsageInfo}
          showStatus={showStatusInHeader}
          showTokenUsage={showTokenUsageInHeader}
        />
      )}

      {/* Scroll container with entries */}
      {groupedBlocks.length > 0 ? (
        <TimelineScrollContainer
          entries={groupedBlocks}
          renderEntry={(block) => (
            <TimelineMessageBlockRenderer
              key={block.id}
              block={block}
              onViewDiff={handleViewDiff}
              activeStreamingId={activeStreamingId}
              showQueuedBadge={
                block.type === 'user' && block.id === lastQueuedUserBlockId
              }
              onEditUserMessage={
                enableEditReset ? handleEditUserMessage : undefined
              }
              onResetUserMessage={
                enableEditReset ? handleResetUserMessage : undefined
              }
            />
          )}
          autoScroll={autoScroll}
          hasOlderEntries={hasMoreOlder}
          isLoadingOlder={isLoadingOlder}
          onLoadOlder={loadOlder}
        />
      ) : (
        <div className="flex-1 flex items-center justify-center bg-background text-sm text-muted-foreground">
          Waiting for agent activity...
        </div>
      )}

      {shouldShowRuntimeTodos && <RuntimeTodosPanel items={runtimeTodos} />}

      {/* Chat input (optional) */}
      {enableChat && onSendMessage && (
        <ChatInputBar
          onSend={onSendMessage}
          disabled={streamState !== 'live' && streamState !== 'stale'}
          placeholder={
            streamState === 'live' || streamState === 'stale'
              ? 'Queue a follow-up for the current attempt...'
              : 'Agent stream is not ready'
          }
        />
      )}

      {/* Diff Viewer Modal */}
      {selectedDiffId && attemptId && (
        <DiffViewerModal
          attemptId={attemptId}
          taskStatus="completed"
          focusFilePath={focusFilePath}
          singleFileMode={Boolean(focusFilePath)}
          onClose={handleCloseDiff}
          onApproved={() => {
            // No-op for timeline view - approval is handled elsewhere
            handleCloseDiff();
          }}
        />
      )}

      {/* Edit User Message Modal */}
      <Dialog open={!!editingEntryId} onOpenChange={(open) => !open && handleCloseEditModal()}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Edit message</DialogTitle>
            <DialogDescription>
              Update the content of your message. The timeline will refresh after saving.
            </DialogDescription>
          </DialogHeader>
          <textarea
            className="min-h-[120px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            value={editingContent}
            onChange={(e) => setEditingContent(e.target.value)}
            placeholder="Message content..."
          />
          <DialogFooter>
            <Button
              variant="outline"
              onClick={handleCloseEditModal}
              disabled={isSavingEdit}
            >
              Cancel
            </Button>
            <Button onClick={handleSaveEdit} disabled={isSavingEdit}>
              {isSavingEdit ? 'Saving...' : 'Save'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Reset coming soon dialog */}
      <Dialog open={resetComingSoonOpen} onOpenChange={setResetComingSoonOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Reset to message</DialogTitle>
            <DialogDescription>
              Reset to this message will remove all content after this point and restart the attempt. This feature is coming soon.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button onClick={() => setResetComingSoonOpen(false)}>OK</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function isTodoToolEntry(entry: TimelineEntry): boolean {
  if (entry.type !== 'tool_call') return false;
  const actionType = (entry as {
    actionType?: { action?: string; todos?: unknown; arguments?: unknown };
  }).actionType;
  if (!actionType) return false;
  if (actionType.action === 'todo_management') return true;
  return parseTodoItems(actionType.todos ?? actionType.arguments).length > 0;
}

export function extractLatestRuntimeTodos(entries: TimelineEntry[]): TodoSummaryItem[] {
  const todoEntries = entries
    .filter((entry): entry is TimelineEntry & {
      actionType?: { action?: string; todos?: unknown; arguments?: unknown };
    } => entry.type === 'tool_call')
    .map((entry) => ({
      entry,
      timestamp: Date.parse(entry.timestamp) || 0,
    }))
    .sort((a, b) => a.timestamp - b.timestamp);

  const mergedTodos = new Map<string, TodoSummaryItem>();
  const orderedKeys: string[] = [];

  for (const { entry } of todoEntries) {
    const actionType = entry.actionType;
    if (!actionType) continue;

    const todos = parseTodoItems(actionType.todos ?? actionType.arguments);
    if (todos.length === 0) continue;

    for (const todo of todos) {
      if (!mergedTodos.has(todo.content)) {
        orderedKeys.push(todo.content);
      }
      mergedTodos.set(todo.content, todo);
    }
  }

  return orderedKeys
    .map((key) => mergedTodos.get(key))
    .filter((value): value is TodoSummaryItem => Boolean(value));
}

function RuntimeTodosPanel({ items }: { items: TodoSummaryItem[] }) {
  const [isOpen, setIsOpen] = useState(() => {
    const stored = localStorage.getItem(TODO_PANEL_OPEN_KEY);
    return stored === null ? true : stored === 'true';
  });

  useEffect(() => {
    localStorage.setItem(TODO_PANEL_OPEN_KEY, String(isOpen));
  }, [isOpen]);

  return (
    <details
      className="group border-t border-dashed border-border bg-background"
      open={isOpen}
      onToggle={(event) => setIsOpen(event.currentTarget.open)}
    >
      <summary className="list-none cursor-pointer">
        <div className="bg-muted/20 px-3 py-2.5 text-sm flex items-center justify-between">
          <span className="font-medium text-foreground">Todos ({items.length})</span>
          <ChevronUp className="h-4 w-4 text-muted-foreground transition-transform group-open:rotate-180" />
        </div>
      </summary>
      <div className="px-3 pb-3">
        <ul className="space-y-2">
          {items.map((item, index) => (
            <li key={`${item.content}-${index}`} className="flex items-start gap-2">
              <span className="mt-0.5 h-4 w-4 flex items-center justify-center shrink-0">
                <TodoStatusIcon status={item.status} />
              </span>
              <span className="text-sm leading-5 break-words">
                {item.status === 'cancelled' ? (
                  <s className="text-muted-foreground">{item.content}</s>
                ) : (
                  item.content
                )}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </details>
  );
}

function TodoStatusIcon({ status }: { status: TodoSummaryItem['status'] }) {
  if (status === 'completed') {
    return <Check className="h-4 w-4 text-success" />;
  }
  if (status === 'in_progress') {
    return <CircleDot className="h-4 w-4 text-blue-500" />;
  }
  if (status === 'cancelled') {
    return <Circle className="h-4 w-4 text-gray-400" />;
  }
  return <Circle className="h-4 w-4 text-muted-foreground" />;
}
