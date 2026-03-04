# SRS: Dashboard

## 1. Introduction
The Dashboard provides users with a high-level overview of the entire system, highlighting active projects, recent agent activity, and high-priority human tasks.

## 2. Access Control
- **Roles**: All authenticated users can view the dashboard.
- **Permissions**: Data visibility is subject to project-level visibility settings (standard users see assigned projects, admins see all).

## 3. UI Components
- **Stat Cards**: Dynamic counters for Projects, Open Tasks, Running Agents, and Sprints.
- **Projects Preview**: List of the 5 most recently active projects.
- **Agent Feed**: Live feed of the last 10 agent log entries.
- **Human Tasks**: Cards for tasks requiring "Human Review".

## 4. Functional Requirements

### [SRS-DSH-001] Load Initial State
- **Trigger**: Screen navigation or manual refresh.
- **Input**: User Session Token (JWT).
- **Output**: Populated stat cards and tables.
- **System Logic**: Calls `GET /api/v1/dashboard` (API trả về stats + projects + agent logs + human tasks trong một payload).
- **Validation**: If any endpoint fails, show a "Partial Data" warning or retry toast.

### [SRS-DSH-002] Create New Project Shortcut
- **Trigger**: Click "New Project" button header.
- **Input**: None (redirect trigger).
- **Output**: Navigation to `/projects` with the "Create Project" modal pre-opened.
- **System Logic**: Updates application state/URL to trigger the modal on the target page.

### [SRS-DSH-003] Navigate to Project Detail
- **Trigger**: Click on a project row in the "Recent Projects" table.
- **Input**: `project_id`.
- **Output**: Navigation to `/projects/:id`.
- **System Logic**: Standard router navigation.

### [SRS-DSH-004] Real-time Agent Feed Update
- **Trigger**: New log event from WebSocket.
- **Input**: SSE/WebSocket payload.
- **Output**: Newest log line appears at the top of the feed with a "highlight" animation.
- **System Logic**: Appends new line to local state buffer; evicts lines older than index 50 to maintain performance.

### [SRS-DSH-005] Claim Human Task
- **Trigger**: Click "Review" on a Human Task card.
- **Input**: `task_id`.
- **Output**: Navigation to the Task Detail page within the Review tab.
- **System Logic**: Navigates to `/tasks/:id?tab=review`.

## 5. Non-Functional Requirements
- **Performance**: Initial dashboard load (3 API calls) must complete in < 400ms under standard network conditions.
- **Real-time**: Log latency from agent to dashboard feed must be < 1s.
- **Responsiveness**: Stat cards must collapse to a vertical stack on mobile viewports (< 768px).
