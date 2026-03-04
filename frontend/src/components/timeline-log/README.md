# Timeline Log Display Components

A complete timeline log display system with card-based visual timeline, operation grouping, nested subagents, interactive chat, and smooth animations.

## Overview

This system provides a rich, interactive timeline view for agent execution history. It includes:

- **Real-time streaming** via Server-Sent Events (SSE)
- **Chat Composer UI** with Avatar-based layout
- **Operation grouping** for consecutive similar operations
- **Interactive chat** for bidirectional communication
- **Markdown & File Diff Viewer** support
- **Normalized JSON parsing** as priority (fallback to Regex available)

## Components

### Container Components

#### `TimelineLogDisplay`
Main container component that orchestrates the entire timeline display.

```tsx
import { TimelineLogDisplay } from '@/components/timeline-log';

<TimelineLogDisplay
  attemptId="attempt-123"
  onSendMessage={async (msg) => { /* handle message */ }}
  enableChat={true}
/>
```

**Props:**
- `attemptId` (string | undefined) - The attempt ID to stream logs for
- `onSendMessage` ((message: string) => Promise<void>) - Callback for chat messages
- `enableChat` (boolean) - Whether to show chat input bar (default: false)

#### `TimelineHeader`
Status bar showing streaming indicator, auto-scroll toggle, and entry count.

#### `TimelineScrollContainer`
Virtualized scroll container with timeline connection line and auto-scroll.

#### `TimelineEntryList`
Entry renderer with animations (alternative to scroll container for non-virtualized lists).

### Entry Rows (Avatar Chat UI)

The timeline renders each log as a Chat Row (avatar + full width content).

#### `OperationGroupRows`
Collapsible card for aggregated operations (3+ consecutive operations of same type).

#### `SubagentRows`
Nested subagent display with recursive timeline.

#### `ToolCallRows`
Enhanced tool call display.
**Features:**
- Tool-specific icon (as Avatar)
- Status badge (running/completed/failed)
- Integrated Runtime Todos list
- Detailed Terminal output blocks

#### `User / Assistant Messages`
Avatar cards for user chat messages and assistant thoughts/responses.

#### `Thinking Process`
Displays agent's internal reasoning with collapsible Thought Process card.

### Utility Components

#### `ChatInputBar`
Interactive input with auto-expanding textarea.

**Features:**
- Min 48px, max 120px height
- Enter to send, Shift+Enter for newline
- Character count indicator (max 2000)
- Loading state
- Connection status indicator

#### `TimelineEntryRenderer`
Router component that renders correct card based on entry type.

## Hooks

### `useTimelineStream`
Main hook that combines streaming, parsing, grouping, and subagent detection.

```tsx
const {
  entries,
  isStreaming,
  isLoading,
  error,
  autoScroll,
  setAutoScroll,
  reconnect,
} = useTimelineStream({
  attemptId: 'attempt-123',
  enableGrouping: true,
  enableSubagentDetection: true,
  enableAutoScroll: true,
});
```

### `useOperationGrouping`
Groups consecutive operations (Read/Grep/Glob) into operation groups.

**Grouping rules:**
- 3+ consecutive operations of same type
- Operations within 5 seconds of each other
- Supported types: file_read, search, file_edit

### `timeline-parsers.ts` (Utilities)
Extract legacy Regex parsing and JSON fallback routines for stable handling of CLI and SDK log streams.

## Styling

### Design Tokens

Uses CSS variables from `index.css`:

- Timeline line: `absolute left-8 top-0 bottom-0 w-0.5 bg-border/40`
- Timeline dots: `w-3 h-3 rounded-full border-2 border-background`
- Card hover: `hover:border-primary/30 transition-colors`

### Color Themes

- **Operation Groups**: Info (blue) - `bg-info`, `text-info`, `border-info`
- **Subagents**: Purple - `bg-purple-500`, `text-purple-500`
- **User Messages**: Primary - `bg-primary`, `text-primary`
- **Assistant Messages**: Success (green) - `bg-success`, `text-success`
- **Thinking**: Purple-400 - `bg-purple-400`, `text-purple-400`
- **Errors**: Destructive (red) - `bg-destructive`, `text-destructive`

Uses standard Tailwind classes and conditional CSS for collapse/expand animations rather than heavy libraries.

## Usage Examples

### Basic Timeline Display

```tsx
import { TimelineLogDisplay } from '@/components/timeline-log';

function AttemptLogsPage() {
  const { attemptId } = useParams();

  return (
    <div className="h-screen">
      <TimelineLogDisplay attemptId={attemptId} />
    </div>
  );
}
```

### With Chat Support

```tsx
import { TimelineLogDisplay } from '@/components/timeline-log';
import { sendUserMessage } from '@/api/attempts';

function InteractiveAgentView() {
  const { attemptId } = useParams();

  const handleSendMessage = async (message: string) => {
    await sendUserMessage(attemptId, message);
  };

  return (
    <div className="h-screen">
      <TimelineLogDisplay
        attemptId={attemptId}
        onSendMessage={handleSendMessage}
        enableChat={true}
      />
    </div>
  );
}
```

### Custom Timeline with Hooks

```tsx
import { useTimelineStream } from '@/hooks/useTimelineStream';
import { TimelineEntryRenderer } from '@/components/timeline-log';

function CustomTimeline({ attemptId }: { attemptId: string }) {
  const { entries, isStreaming, error } = useTimelineStream({
    attemptId,
    enableGrouping: true,
    enableSubagentDetection: true,
  });

  if (error) return <div>Error: {error}</div>;

  return (
    <div className="space-y-3 p-4">
      {entries.map((entry) => (
        <TimelineEntryRenderer key={entry.id} entry={entry} />
      ))}
    </div>
  );
}
```

## Accessibility

All components include:
- Proper ARIA labels
- Keyboard navigation support
- Focus management
- Screen reader friendly

## Performance

- **Virtualized rendering** using `react-virtuoso` for large lists
- **Operation grouping** reduces DOM nodes for repetitive operations
- **Lazy loading** with SSE streaming
- **Optimized re-renders** with useMemo and useCallback

## Dependencies

- `react` - Core framework
- `lucide-react` - Icons
- `react-virtuoso` - Virtualized scrolling
- `react-markdown` - Formatting Thought process
- Existing utilities: `@/lib/utils`, `@/utils/formatters`, `@/utils/icon-mapping`

## Type Safety

All components are fully typed with TypeScript. See `@/types/timeline-log` for type definitions.

## Future Enhancements

- [ ] File change card component
- [ ] Search/filter timeline entries
- [ ] Export timeline to JSON/HTML
- [ ] Keyboard shortcuts
- [ ] Theming support
- [ ] Performance metrics overlay
