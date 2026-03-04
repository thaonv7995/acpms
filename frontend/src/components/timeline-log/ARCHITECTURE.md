# Timeline Log Architecture

## Visual Component Tree

```
┌─────────────────────────────────────────────────────────────┐
│                    TimelineLogDisplay                       │
│  (Root container - orchestrates everything)                 │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌──────────────┐  ┌──────────────────┐  ┌──────────────┐
│TimelineHeader│  │TimelineScroll    │  │ChatInputBar  │
│              │  │Container         │  │(optional)    │
└──────────────┘  └──────────────────┘  └──────────────┘
                           │
                           ▼
                  ┌─────────────────┐
                  │ Virtuoso List   │
                  │ (react-virtuoso)│
                  └─────────────────┘
                           │
                           ▼
            ┌──────────────────────────────┐
            │  TimelineEntryRenderer       │
            │  (Routes to correct card)    │
            └──────────────────────────────┘
                           │
         ┏━━━━━━━━━━━━━━━━━━┻━━━━━━━━━━━━━━━━━━┓
         ┃                                      ┃
         ┃  Entry Renderer via `LogLine` ChatRow┃
         ┃                                      ┃
         ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
                            │
      ┌─────────┬───────┬───┴────┬──────┬─────────┐
      ▼         ▼       ▼        ▼      ▼         ▼
 ┌─────────┐ ┌────┐ ┌─────┐ ┌──────┐ ┌──────┐ ┌──────┐
 │Operation│ │Sub │ │Tool │ │Chat  │ │Think │ │File  │
 │GroupRows│ │agent│ │Call │ │Msgs  │ │Card  │ │Card  │
 └─────────┘ └────┘ └─────┘ └──────┘ └──────┘ └──────┘
                 │
                 ▼
         ┌───────────────┐
         │   Recursive   │
         │   Timeline    │
         │  (nested)     │
         └───────────────┘
```

## Data Flow

```
┌──────────────────────────────────────────────────────────────┐
│                     Backend (Rust)                           │
│  ┌──────────┐      ┌──────────┐      ┌──────────┐         │
│  │ Agent    │ ───▶ │ Log      │ ───▶ │ SSE      │         │
│  │ Executor │      │ Collector│      │ Stream   │         │
│  └──────────┘      └──────────┘      └──────────┘         │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼ (Server-Sent Events)
┌──────────────────────────────────────────────────────────────┐
│                   Frontend (React)                           │
│                                                              │
│  ┌────────────────────────────────────────────────┐         │
│  │        useAttemptStream (Hook)                 │         │
│  │  • Connects to SSE endpoint                    │         │
│  │  • Handles reconnection                        │         │
│  │  • Buffers raw log entries                     │         │
│  └────────────────────────────────────────────────┘         │
│                         │                                    │
│                         ▼                                    │
│  ┌────────────────────────────────────────────────┐         │
│  │        useTimelineStream (Hook)                │         │
│  │  1. Parse logs → Timeline entries              │         │
│  │  2. Detect subagents                           │         │
│  │  3. Group operations                           │         │
│  └────────────────────────────────────────────────┘         │
│                         │                                    │
│                         ▼                                    │
│  ┌────────────────────────────────────────────────┐         │
│  │     TimelineLogDisplay (Component)             │         │
│  │  • Renders header, scroll, chat                │         │
│  │  • Manages auto-scroll state                   │         │
│  │  • Handles user interactions                   │         │
│  └────────────────────────────────────────────────┘         │
│                         │                                    │
│                         ▼                                    │
│  ┌────────────────────────────────────────────────┐         │
│  │     TimelineEntryRenderer (Router)             │         │
│  │  • Routes entry → correct card component       │         │
│  └────────────────────────────────────────────────┘         │
│                         │                                    │
│                         ▼                                    │
│            ┌────────────────────────┐                       │
│            │   Card Components       │                       │
│            │  (Render UI)           │                       │
│            └────────────────────────┘                       │
└──────────────────────────────────────────────────────────────┘
```

## Hook Processing Pipeline

```
Raw Log Entry (from SSE)
        │
        ▼
┌───────────────────┐
│ parseLogEntry()   │  Parse JSON content, extract metadata
└───────────────────┘
        │
        ▼
┌───────────────────┐
│ TimelineEntry     │  Base typed entry (tool_call, message, etc.)
└───────────────────┘
        │
        ▼
┌───────────────────────────┐
│ useSubagentDetection()    │  Task tool → SubagentEntry
└───────────────────────────┘
        │
        ▼
┌───────────────────────────┐
│ useOperationGrouping()    │  3+ consecutive ops → OperationGroup
└───────────────────────────┘
        │
        ▼
┌───────────────────────────┐
│ Final Timeline Entries    │  Ready for rendering
└───────────────────────────┘
```

## Operation Grouping Logic

```
Input: [Read, Read, Read, Edit, Search, Search, Search, Search]
                                    │
                                    ▼
        ┌───────────────────────────────────────────────┐
        │  Grouping Rules:                              │
        │  1. Same action type                          │
        │  2. Within 5 seconds                          │
        │  3. Minimum 3 operations                      │
        └───────────────────────────────────────────────┘
                                    │
                                    ▼
Output: [OperationGroup(Read×3), Edit, OperationGroup(Search×4)]
```

## State Management

```
┌─────────────────────────────────────────────────────┐
│              TimelineLogDisplay                     │
│                                                     │
│  Local State:                                       │
│  • autoScroll: boolean                              │
│  • (inherited from useTimelineStream)               │
│                                                     │
│  From useTimelineStream:                            │
│  • entries: TimelineEntry[]                         │
│  • isStreaming: boolean                             │
│  • isLoading: boolean                               │
│  • error: string | null                             │
│                                                     │
│  From useAttemptStream (internal):                  │
│  • logs: LogEntry[]                                 │
│  • attempt: AttemptState | null                     │
│  • isConnected: boolean                             │
│                                                     │
│  Card State (per component):                        │
│  • expanded: boolean (for collapsible cards)        │
└─────────────────────────────────────────────────────┘
```

## Entry Type Routing

```
TimelineEntryRenderer receives entry.type
                │
                ▼
        ┌───────────────┐
        │  Switch on    │
        │  entry.type   │
        └───────────────┘
                │
    ┌───────────┼───────────────────────────┐
    │           │           │               │
    ▼           ▼           ▼               ▼
tool_call   operation   subagent    user_message
    │        _group         │               │
    ▼           │           ▼               ▼
ToolCall    Operation   Subagent       LogLine
Rows        GroupRows   Rows           (ChatRow)
```

## Timeline Visual Structure

```
┌──────────────────────────────────────────────────┐
│                                                  │
│  ┌───────────────────────────────────────────┐  │
│  │           Timeline Header                  │  │
│  │  ● Streaming  |  12 entries  |  Auto ✓   │  │
│  └───────────────────────────────────────────┘  │
│                                                  │
│  ┌───────────────────────────────────────────┐  │
│  │                                           │  │
│  │  avatar ┌─────────────────────────────────┐│  │
│  │     (A) │  Assistant Chat Message         ││  │
│  │         └─────────────────────────────────┘│  │
│  │                                           │  │
│  │  avatar ┌─────────────────────────────────┐│  │
│  │     (U) │  User Message                   ││  │
│  │         └─────────────────────────────────┘│  │
│  │                                           │  │
│  │  avatar ┌─────────────────────────────────┐│  │
│  │     (T) │  Tool Call (Edit)               ││  │
│  │         │   ┌───────────────────────────┐ ││  │
│  │         │   │  Terminal command details │ ││  │
│  │         │   └───────────────────────────┘ ││  │
│  │         └─────────────────────────────────┘│  │
│  │                                           │  │
│  │  avatar ┌─────────────────────────────────┐│  │
│  │     (T) │  Subagent (Task spawn)          ││  │
│  │         │   ┌───────────────────────────┐ ││  │
│  │         │   │ Nested timeline...        │ ││  │
│  │         │   └───────────────────────────┘ ││  │
│  │         └─────────────────────────────────┘│  │
│  │                                           │  │
│  │           (Virtualized scroll)            │  │
│  └───────────────────────────────────────────┘  │
│                                                  │
│  ┌───────────────────────────────────────────┐  │
│  │  [Type message...]              [Send]    │  │
│  │  Press Enter to send ⏎                    │  │
│  └───────────────────────────────────────────┘  │
│                                                  │
└──────────────────────────────────────────────────┘

Legend:
┃ = Timeline connection line (left edge)
● = Timeline dot (per entry)
□ = Card with border
```

## Performance Optimizations

```
┌────────────────────────────────────────────────────┐
│  Optimization Strategy                             │
├────────────────────────────────────────────────────┤
│                                                    │
│  1. Virtualization (react-virtuoso)                │
│     • Only render visible entries                  │
│     • 200px overscan                               │
│     • Reduces DOM nodes from 1000+ to ~20         │
│                                                    │
│  2. Operation Grouping                             │
│     • 3+ ops → 1 group card                       │
│     • Reduces re-renders by 3x                     │
│     • Lazy render children when collapsed          │
│                                                    │
│  3. Memoization                                    │
│     • useMemo for expensive transforms             │
│     • useCallback for event handlers               │
│     • React.memo for card components               │
│                                                    │
│  4. SSE Optimizations                              │
│     • Incremental JSON patch updates               │
│     • Sequence number tracking                     │
│     • Automatic reconnection with backoff          │
│                                                    │
│  5. Animation Optimizations                        │
│     • CSS transitions for Chat Row expands         │
│     • Staggered animations (max 0.5s delay)        │
│                                                    │
└────────────────────────────────────────────────────┘
```

## Error Handling

```
┌─────────────────────────────────────────────────┐
│  Error Boundary Hierarchy                       │
├─────────────────────────────────────────────────┤
│                                                 │
│  TimelineLogDisplay                             │
│    ├─ Connection errors → Error UI              │
│    ├─ Parse errors → Skip entry                 │
│    └─ Component errors → Fallback card          │
│                                                 │
│  useTimelineStream                              │
│    ├─ SSE connection failure → Retry            │
│    ├─ JSON parse error → Log & skip             │
│    └─ Gap detected → Full resync                │
│                                                 │
│  Card Components                                │
│    └─ Render error → Unknown entry card         │
│                                                 │
└─────────────────────────────────────────────────┘
```

## Integration Points

```
┌──────────────────────────────────────────────────┐
│  External APIs & Services                        │
├──────────────────────────────────────────────────┤
│                                                  │
│  Input:                                          │
│  • SSE: /api/v1/attempts/:id/stream             │
│  • Types: @/types/timeline-log                  │
│                                                  │
│  Output:                                         │
│  • Chat: onSendMessage callback                 │
│  • Events: User interactions                    │
│                                                  │
│  Utilities:                                      │
│  • formatTimestamp (date formatting)            │
│  • getActionIcon (icon mapping)                 │
│  • cn (className utility)                       │
│                                                  │
└──────────────────────────────────────────────────┘
```

## File Dependencies Graph

```
TimelineLogDisplay.tsx
  ├─ TimelineHeader.tsx
  ├─ TimelineScrollContainer.tsx
  │   ├─ react-virtuoso
  │   └─ useAutoScroll
  ├─ TimelineEntryRenderer.tsx
  │   ├─ (Contains internal render functions:)
  │   ├─ OperationGroupRows
  │   ├─ SubagentRows
  │   │   └─ TimelineEntryRenderer.tsx (recursive)
  │   ├─ ToolCallRows
  │   └─ LogLine (Base ChatRow layout)
  ├─ ChatInputBar.tsx
  └─ useTimelineStream
      ├─ timeline-parsers.ts
      ├─ useAttemptStream (existing)
      ├─ useSubagentDetection
      └─ useOperationGrouping

Shared Dependencies:
  • @/lib/utils (cn)
  • @/utils/formatters (formatTimestamp)
  • @/utils/icon-mapping (getActionIcon)
  • @/types/timeline-log (all types)
  • lucide-react (icons)
```
