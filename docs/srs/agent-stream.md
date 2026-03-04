# SRS: Agent Stream

## 1. Introduction
The Agent Stream is a centralized real-time monitoring center that aggregates activity and log data from all active agents across the platform into a single unified view.

## 2. Access Control
- **Roles**: All authenticated users.
- **Permissions**:
  - `admin` / `PO`: Can view all global streams.
  - `developer` / `QA`: Can view streams for projects they are assigned to.

## 3. UI Components
- **Global Status Bar**: Real-time counters (Running, Success, Error).
- **Infinite Log Console**: High-performance virtualization-based terminal.
- **Agent Context Switcher**: Side-bar list of active agent attempts.
- **Filter Toolbar**: Search, Status filter, and Logging level toggle (Info, Debug, Error).

## 4. Functional Requirements

### [SRS-STR-001] Aggregate Multi-Agent Logs
- **Trigger**: Screen navigation.
- **Input**: JWT, Filter selection.
- **Output**: Merged stream of logs from all active attempts.
- **System Logic**: Kết hợp `GET /api/v1/agent-activity/logs` (initial) + WebSocket project stream (`/ws/projects/:project_id/agents`) cho realtime.
- **Validation**: Ensure log lines are interleaved correctly using high-resolution timestamps.

### [SRS-STR-002] Focus on Specific Attempt
- **Trigger**: Click an agent card in the switcher.
- **Input**: `attempt_id`.
- **Output**: Terminal filters to show only selected agent logs.
- **System Logic**: Client-side filtering of the incoming log buffer.

### [SRS-STR-003] Scroll Lock & Auto-Follow
- **Trigger**: User manual scroll up (Lock) or scroll to bottom (Follow).
- **Input**: Scroll position.
- **Output**: Terminal behavior adjustment.
- **System Logic**: Managed by UI state controller for the virtual list.

### [SRS-STR-004] Quick Review Redirect
- **Trigger**: Click "Review" button on an agent card (visible when agent is done).
- **Input**: `task_id`.
- **Output**: Navigation to the specific Task Detail page.
- **System Logic**: Standard router navigation.

### [SRS-STR-005] Log Search & Regex
- **Trigger**: Typing in the stream search box.
- **Input**: String or Regex.
- **Output**: Highlights filtered matches in the terminal.
- **System Logic**: Purely client-side for performance.

## 5. Non-Functional Requirements
- **Performance**: Must handle sustained log ingest of 2000 lines/second without locking the UI main thread.
- **Memory Management**: Local log buffer capped at 5000 lines per attempt to prevent browser memory exhaustion.
