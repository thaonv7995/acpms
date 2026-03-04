# UI Flow: Dashboard

The Dashboard provides a high-level overview of the system state, active projects, and urgent tasks.

## Screen Overview
- **Stats Row**: Real-time counters for projects, agents, system load, and PRs.
- **Projects Table**: Quick access to active projects.
- **Agent Feed**: Live log stream from all active agents.
- **Tasks Sidebar**: List of tasks requiring human intervention (Review, Approve, Assign).

## Interactive Elements & Actions

### 1. Stats Cards
- **Elements**: 4 Cards (Active Projects, Agents Online, System Load, Pending PRs).
- **Data Source**: `useDashboard` hook calls `GET /api/v1/dashboard`.
- **Action**: Visual only (Navigation happens by clicking "View All" redirects).

### 2. Projects Table
- **Action**: Click a project row.
- **Result**: Navigates to `/projects/:id`.

### 3. Agent Live Feed
- **Element**: `AgentFeed` component.
- **Data Source**: `GET /api/v1/dashboard` (initial) + WebSocket/SSE streams cho cập nhật real-time.
- **Action**: Click a log line.
- **Result**: Navigates to the corresponding Task Detail page.

### 4. Tasks Sidebar
- **Action: "Review" Button**
    - **Trigger**: Click "Review" on a task card.
    - **Effect**: `navigate("/tasks?taskId={id}&action=review")`.
- **Action: "Approve" Button**
    - **Trigger**: Click "Approve" on a task card.
    - **Effect**: `navigate("/tasks?taskId={id}&action=approve")`.
- **Action: "Add Task" Button**
    - **Trigger**: Click "+ Add Task" at the bottom of the sidebar.
    - **Effect**: Opens `CreateTaskModal`. Calls `POST /api/v1/tasks` upon submission.
    - **Backend**: `crates/server/src/routes/tasks.rs:create_task`.
