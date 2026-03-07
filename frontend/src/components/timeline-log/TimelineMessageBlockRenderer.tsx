import type { TimelineEntry, FileChangeEntry } from '@/types/timeline-log';
import { TimelineEntryRenderer, ThinkingGroupRenderer } from './TimelineEntryRenderer';
import { ChatAggregatedFileChanges, type AggregatedFileChange } from './ChatAggregatedFileChanges';
import { cn } from '@/lib/utils';
import { formatLogPathForConversation } from '@/lib/logPathDisplay';
import { memo, useMemo, useState } from 'react';

export interface MessageBlock {
    id: string;
    type: 'user' | 'assistant';
    entries: TimelineEntry[];
}

export function groupEntriesIntoBlocks(entries: TimelineEntry[]): MessageBlock[] {
    const blocks: MessageBlock[] = [];
    let currentBlock: MessageBlock | null = null;

    for (const entry of entries) {
        if (entry.type === 'user_message') {
            if (currentBlock) blocks.push(currentBlock);
            blocks.push({ id: entry.id, type: 'user', entries: [entry] });
            currentBlock = null;
        } else {
            if (!currentBlock || currentBlock.type === 'user') {
                if (currentBlock) blocks.push(currentBlock);
                currentBlock = { id: `ai-block-${entry.id}`, type: 'assistant', entries: [entry] };
            } else {
                currentBlock.entries.push(entry);
            }
        }
    }
    if (currentBlock) blocks.push(currentBlock);
    return blocks;
}

type DisplayItem =
  | { type: 'entry'; entry: TimelineEntry }
  | { type: 'thinking_group'; entries: Array<{ content: string; expansionKey: string }> }
  | { type: 'file_group'; file: AggregatedFileChange };

function getChangeLabel(changeType: string): string {
  const t = changeType.toLowerCase();
  if (t === 'created' || t === 'added') return 'Created';
  if (t === 'deleted') return 'Deleted';
  if (t === 'renamed') return 'Renamed';
  return 'Edited';
}

function processBlockEntries(entries: TimelineEntry[]): DisplayItem[] {
  const result: DisplayItem[] = [];
  let thinkingBuffer: Array<{ id: string; content: string }> = [];
  let fileBuffer: FileChangeEntry[] = [];
  const fileToolDisplayPaths = new Set(
    entries.flatMap((entry) => {
      if (entry.type !== 'tool_call') return [];
      const action = entry.actionType?.action;
      if (action !== 'file_edit' && action !== 'file_write') return [];

      const path = entry.actionType?.file_path || entry.actionType?.path;
      if (typeof path !== 'string' || !path.trim()) return [];

      return [formatLogPathForConversation(path)];
    })
  );

  const flushThinking = () => {
    if (thinkingBuffer.length > 0) {
      result.push({
        type: 'thinking_group',
        entries: thinkingBuffer.map((e, i) => ({
          content: e.content,
          expansionKey: e.id || `thinking-${i}`,
        })),
      });
    }
    thinkingBuffer = [];
  };

  const flushFileGroup = () => {
    if (fileBuffer.length === 0) return;
    const first = fileBuffer[0];
    const path = first.path;
    if (fileToolDisplayPaths.has(formatLogPathForConversation(path))) {
      fileBuffer = [];
      return;
    }
    let linesAdded = 0;
    let linesRemoved = 0;
    for (const e of fileBuffer) {
      linesAdded += typeof e.linesAdded === 'number' ? Math.max(0, e.linesAdded) : 0;
      linesRemoved += typeof e.linesRemoved === 'number' ? Math.max(0, e.linesRemoved) : 0;
    }
    const changeType = typeof first.changeType === 'string' ? first.changeType : 'modified';
    result.push({
      type: 'file_group',
      file: {
        path,
        changeLabel: getChangeLabel(changeType),
        linesAdded,
        linesRemoved,
        diffId: first.diffId ?? `file:${path}`,
      },
    });
    fileBuffer = [];
  };

  for (const entry of entries) {
    if (entry.type === 'thinking') {
      flushFileGroup();
      thinkingBuffer.push({ id: entry.id, content: entry.content || '' });
    } else if (entry.type === 'file_change') {
      flushThinking();
      const fc = entry as FileChangeEntry;
      if (fileBuffer.length > 0 && fileBuffer[fileBuffer.length - 1].path !== fc.path) {
        flushFileGroup();
      }
      fileBuffer.push(fc);
    } else {
      flushThinking();
      flushFileGroup();
      result.push({ type: 'entry', entry });
    }
  }
  flushThinking();
  flushFileGroup();
  return result;
}

interface TimelineMessageBlockRendererProps {
    block: MessageBlock;
    onViewDiff?: (diffId: string, filePath?: string) => void;
    activeStreamingId?: string | null;
    onEditUserMessage?: (entryId: string, content: string) => void;
    onResetUserMessage?: (entryId: string) => void;
    /** When true, user message shows "Queued" badge - agent will process when current task completes */
    showQueuedBadge?: boolean;
}

function TimelineMessageBlockRendererComponent({
    block,
    onViewDiff,
    activeStreamingId,
    onEditUserMessage,
    onResetUserMessage,
    showQueuedBadge = false,
}: TimelineMessageBlockRendererProps) {
    const isUser = block.type === 'user';
    const displayItems = useMemo(() => processBlockEntries(block.entries), [block.entries]);

    return (
        <div className={cn("flex w-full px-6 py-4 border-b border-border/40", isUser ? 'bg-muted/10' : 'bg-transparent')}>
            <div className="flex-1 min-w-0 flex flex-col gap-2">
                {displayItems.map((item, index) => {
                    if (item.type === 'thinking_group') {
                        return (
                            <ThinkingGroupItem
                                key={`thinking-group-${index}`}
                                entries={item.entries}
                            />
                        );
                    }
                    if (item.type === 'file_group') {
                        return (
                            <ChatAggregatedFileChanges
                                key={`file-${item.file.path}-${index}`}
                                file={item.file}
                                onViewDiff={onViewDiff}
                            />
                        );
                    }
                    const entry = item.entry;
                    return (
                        <TimelineEntryRenderer
                            key={entry.id || index}
                            entry={entry}
                            onViewDiff={onViewDiff}
                            hideAvatarAndBorder
                            isStreaming={activeStreamingId != null && entry.id === activeStreamingId}
                            onEditUserMessage={onEditUserMessage}
                            onResetUserMessage={onResetUserMessage}
                            showQueuedBadge={entry.type === 'user_message' ? showQueuedBadge : undefined}
                        />
                    );
                })}
            </div>
        </div>
    );
}

function ThinkingGroupItem({ entries }: { entries: Array<{ content: string; expansionKey: string }> }) {
    const [expanded, setExpanded] = useState(false);
    return (
        <ThinkingGroupRenderer
            entries={entries}
            expanded={expanded}
            onToggle={() => setExpanded((v) => !v)}
        />
    );
}

export const TimelineMessageBlockRenderer = memo(TimelineMessageBlockRendererComponent);
