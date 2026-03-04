# SRS: Kanban Board

## 1. Introduction
The Kanban Board is a high-performance, real-time interface for managing task lifecycles across projects. It supports drag-and-drop operations, split-view monitoring, and multi-panel layout transitions.

## 2. Access Control
- **Roles**: All authenticated users.
- **Permissions**: 
  - `dev` / `QA`: Can move tasks assigned to them or marked as "Ready for Review".
  - `admin` / `PO`: Full movement authority across all columns.

## 3. UI Components
- **Column System**: Todo, In Progress, Review, Done (Infinite scrolling).
- **Task Cards**: Dense information display (Title, Assignee, Priority, Tech Stack).
- **3-Panel Layout**: 
  - Panel 1: Kanban Board.
  - Panel 2: Task Attempt / Logs (SSE).
  - Panel 3: Preview (IFrame) or Diff Viewer.

## 4. Functional Requirements

### [SRS-KAN-001] Load Tasks by Scope
- **Trigger**: Screen navigation or project selection.
- **Input**: `project_id` (or 'all'), `assignee_id`.
- **Output**: Populated Kanban columns.
- **System Logic**: Calls `GET /api/v1/tasks?project_id=...`.
- **Validation**: Empty columns should display a "No tasks" placeholder.

### [SRS-KAN-002] Drag-and-Drop Task Movement
- **Trigger**: User drags a card from one column to another.
- **Input**: `task_id`, `source_status`, `target_status`.
- **Output**: Optimistic UI update followed by success/failure confirmation.
- **System Logic**: Calls `PATCH /api/v1/tasks/:id/status`.
- **Validation**: 
  - Prevents invalid transitions (e.g., Todo -> Done directly if review is required).
  - Reverts UI if API call fails.

### [SRS-KAN-003] Open Split-View Side Panel
- **Trigger**: Click on a Task Card.
- **Input**: `task_id`.
- **Output**: Side panel slides in; Board width adjusts.
- **System Logic**: Updates URL to `/tasks/:id`. Triggers fetch of `GET /api/v1/tasks/:id/attempts`.

### [SRS-KAN-004] Toggle Layout Mode
- **Trigger**: Click layout icons (Single, Split, Full).
- **Input**: Layout type.
- **Output**: UI redistribution.
- **System Logic**: Client-side state change (`zustand` or `useState`).

### [SRS-KAN-005] Real-time Status Sync
- **Trigger**: External status change (e.g., Agent finishes task).
- **Input**: WebSocket `TASK_UPDATED` event.
- **Output**: Task card moves automatically to "Review" or "Done" without refresh.
- **System Logic**: Updates the local task list buffer.

## 5. Non-Functional Requirements
- **Performance**: Must maintain 60FPS during drag-and-drop even with 500+ tasks on board.
- **Virtualization**: Uses virtual lists for column content to handle scaling.
