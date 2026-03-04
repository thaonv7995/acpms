/**
 * Example usage of Timeline Log Display components
 *
 * This file demonstrates how to integrate the timeline log system
 * into your application. It's not meant to be used directly in production,
 * but rather as a reference for implementation.
 */

import { TimelineLogDisplay } from './TimelineLogDisplay';
import { logger } from '@/lib/logger';

/**
 * Example 1: Basic timeline display
 * Shows logs for a specific attempt with no chat
 */
export function BasicTimelineExample() {
  const attemptId = 'attempt-123'; // Replace with actual attempt ID

  return (
    <div className="h-screen bg-background">
      <TimelineLogDisplay attemptId={attemptId} />
    </div>
  );
}

/**
 * Example 2: Timeline with interactive chat
 * Allows users to send messages to the running agent
 */
export function InteractiveTimelineExample() {
  const attemptId = 'attempt-123'; // Replace with actual attempt ID

  const handleSendMessage = async (message: string) => {
    // Send message to agent via WebSocket or API
    logger.log('Sending message:', message);

    // Example API call (replace with your actual implementation):
    // await fetch(`/api/v1/attempts/${attemptId}/messages`, {
    //   method: 'POST',
    //   headers: { 'Content-Type': 'application/json' },
    //   body: JSON.stringify({ content: message }),
    // });
  };

  return (
    <div className="h-screen bg-background">
      <TimelineLogDisplay
        attemptId={attemptId}
        onSendMessage={handleSendMessage}
        enableChat={true}
      />
    </div>
  );
}

/**
 * Example 3: Timeline in a split-panel layout
 * Shows timeline alongside other content
 */
export function SplitPanelTimelineExample() {
  const attemptId = 'attempt-123';

  return (
    <div className="flex h-screen bg-background">
      {/* Left panel: Task details */}
      <div className="w-1/3 border-r border-border p-4 overflow-auto">
        <h1 className="text-2xl font-bold mb-4">Task Details</h1>
        <div className="space-y-4">
          <div>
            <h2 className="text-sm font-medium text-muted-foreground">Status</h2>
            <p className="text-lg">Running</p>
          </div>
          <div>
            <h2 className="text-sm font-medium text-muted-foreground">Started</h2>
            <p className="text-sm">2 minutes ago</p>
          </div>
          {/* Add more task metadata */}
        </div>
      </div>

      {/* Right panel: Timeline */}
      <div className="flex-1">
        <TimelineLogDisplay attemptId={attemptId} />
      </div>
    </div>
  );
}

/**
 * Example 4: Timeline with custom header
 * Adds additional controls above the timeline
 */
export function CustomHeaderTimelineExample() {
  const attemptId = 'attempt-123';

  return (
    <div className="h-screen flex flex-col bg-background">
      {/* Custom header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-border bg-card">
        <h1 className="text-xl font-semibold">Agent Execution Timeline</h1>
        <div className="flex gap-2">
          <button className="px-3 py-1.5 text-sm border border-border rounded-md hover:bg-muted">
            Export
          </button>
          <button className="px-3 py-1.5 text-sm border border-border rounded-md hover:bg-muted">
            Filter
          </button>
        </div>
      </div>

      {/* Timeline */}
      <div className="flex-1">
        <TimelineLogDisplay attemptId={attemptId} />
      </div>
    </div>
  );
}

/**
 * Example 5: Using timeline hooks directly for custom UI
 */
import { useTimelineStream } from '@/hooks/useTimelineStream';
import { TimelineEntryRenderer } from './TimelineEntryRenderer';

export function CustomTimelineExample() {
  const attemptId = 'attempt-123';

  const {
    entries,
    isStreaming,
    isLoading,
    error,
    autoScroll,
    setAutoScroll,
  } = useTimelineStream({
    attemptId,
    enableGrouping: true,
    enableSubagentDetection: true,
    enableAutoScroll: true,
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-muted-foreground">Loading timeline...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-destructive">Error: {error}</div>
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col">
      {/* Custom header */}
      <div className="px-4 py-3 border-b border-border bg-card">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div
              className={`w-2 h-2 rounded-full ${
                isStreaming ? 'bg-success animate-pulse' : 'bg-muted-foreground/40'
              }`}
            />
            <span className="text-sm text-muted-foreground">
              {entries.length} entries
            </span>
          </div>
          <button
            onClick={() => setAutoScroll(!autoScroll)}
            className="text-sm px-3 py-1 rounded-md bg-muted hover:bg-muted/80"
          >
            Auto-scroll: {autoScroll ? 'On' : 'Off'}
          </button>
        </div>
      </div>

      {/* Custom timeline rendering */}
      <div className="flex-1 overflow-auto p-4">
        <div className="max-w-4xl mx-auto space-y-3">
          {entries.map((entry) => (
            <TimelineEntryRenderer key={entry.id} entry={entry} />
          ))}
        </div>
      </div>
    </div>
  );
}

/**
 * Example 6: Timeline with filtering
 * Shows only specific entry types
 */
export function FilteredTimelineExample() {
  const attemptId = 'attempt-123';
  const [filter, setFilter] = React.useState<string>('all');

  const { entries } = useTimelineStream({
    attemptId,
    enableGrouping: true,
    enableSubagentDetection: true,
  });

  const filteredEntries = React.useMemo(() => {
    if (filter === 'all') return entries;
    return entries.filter((entry) => entry.type === filter);
  }, [entries, filter]);

  return (
    <div className="h-screen flex flex-col">
      {/* Filter controls */}
      <div className="px-4 py-3 border-b border-border bg-card">
        <div className="flex gap-2">
          {['all', 'tool_call', 'user_message', 'assistant_message', 'error'].map(
            (type) => (
              <button
                key={type}
                onClick={() => setFilter(type)}
                className={`px-3 py-1.5 text-sm rounded-md transition-colors ${
                  filter === type
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted text-muted-foreground hover:bg-muted/80'
                }`}
              >
                {type.replace('_', ' ')}
              </button>
            )
          )}
        </div>
      </div>

      {/* Timeline */}
      <div className="flex-1 overflow-auto p-4">
        <div className="max-w-4xl mx-auto space-y-3">
          {filteredEntries.map((entry) => (
            <TimelineEntryRenderer key={entry.id} entry={entry} />
          ))}
          {filteredEntries.length === 0 && (
            <div className="text-center text-muted-foreground py-8">
              No entries matching filter
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// Don't forget to add React import for the last examples
import React from 'react';
