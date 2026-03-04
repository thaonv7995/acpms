# SRS: Task Detail & Agent Control

## 1. Introduction
The Task Detail view is the operational command center for a single task. It handles agent orchestration, code review, and metadata management.

## 2. Access Control
- **Roles**: All authenticated users.
- **Permissions**: 
  - `admin` / `PO` / `dev`: Can start/stop agents and approve changes.
  - `viewer`: Read-only access to logs and diffs.

## 3. UI Components
- **Agent Control Area**: Controls for Starting, Stopping, and Restarting agents.
- **Attempt History**: List of past agent runs for this task.
- **Log Viewer**: Real-time terminal with search and scroll-lock.
- **Review Panel**: Side-by-side diff viewer for proposed code changes.

## 4. Functional Requirements

### [SRS-TSK-001] Initiate Agent Run
- **Trigger**: Click "Start Agent".
- **Input**: `task_id`, optional `execution_params`.
- **Output**: New `TaskAttempt` created; Log viewer connects to stream.
- **System Logic**: Calls `POST /api/v1/tasks/:id/attempts`. Worker orchestrator spawns a fresh Docker container.

### [SRS-TSK-002] Real-time Log Streaming
- **Trigger**: Active agent run.
- **Input**: WebSocket `/ws/attempts/:id/logs` hoặc SSE `/api/v1/attempts/:id/stream`.
- **Output**: Terminal updates in real-time.
- **System Logic**: Connects to the event stream; handles line buffering and ANSI color parsing.

### [SRS-TSK-003] Review & Approve Changes
- **Trigger**: Navigation to "Review" tab -> Click "Approve".
- **Input**: `patch_id`.
- **Output**: Task moves to "Done"; Bridge to GitLab MR merge.
- **System Logic**: Calls `POST /api/v1/attempts/:id/approve`. Triggers GitOps sync via orchestrator.

### [SRS-TSK-004] View Preview Environment
- **Trigger**: Navigation to "Preview" tab.
- **Input**: None.
- **Output**: IFrame loading the dynamically generated Cloudflare Tunnel URL.
- **System Logic**: Fetches `GET /api/v1/attempts/:id/preview`.

### [SRS-TSK-005] Update Task Metadata
- **Trigger**: Click edit on Title, Description, or Priority.
- **Input**: Form data.
- **Output**: Updated state.
- **System Logic**: Calls `PUT /api/v1/tasks/:id` hoặc `PUT /api/v1/tasks/:id/metadata`.

## 5. Non-Functional Requirements
- **Terminal UX**: Must support "Follow Logs" (scroll to bottom) with manual override.
- **Diff Performance**: Must handle multi-file diffs (> 1000 lines) efficiently using syntax highlighting workers.
