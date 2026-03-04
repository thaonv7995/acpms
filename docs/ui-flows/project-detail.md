# UI Flow: Project Detail

The Project Detail page is the central hub for a specific repository, providing task management, requirements, and settings.

## Screen Overview
- **Header**: Navigation back, project title, and quick stats.
- **Sprint Selector**: Filters tasks by the current milestone.
- **Tabs**:
    - **Summary**: Health metrics and project description.
    - **Tasks (Kanban)**: The primary workflow area.
    - **Requirements**: Product documentation and specifications.
    - **Architecture**: System design diagrams and notes.
    - **Settings**: Project-level configuration.

## Interactive Elements & Actions

### 1. Global Actions
- **Action: "Add Task" Button**
    - **Modal**: Opens `CreateTaskModal`.
    - **Submission**: Calls `POST /api/v1/tasks`.
    - **Backend**: `crates/server/src/routes/tasks.rs:create_task`.
- **Action: Sprint Selector**
    - **Trigger**: Select a different sprint from the dropdown.
    - **Effect**: Updates `selectedSprintId`, triggering a refetch of tasks matching that sprint.
    - **Backend**: `GET /api/v1/tasks?project_id=:id&sprint_id=...`.

### 2. Tasks Tab (Kanban)
- **Action: Click Task Card**
    - **Effect**: Navigates to `/projects/:projectId/task/:taskId`.
- **Action: "View Logs" (Icon)**
    - **Modal**: Opens `ViewLogsModal`.
    - **Logic**: Fetches latest attempt via `GET /api/v1/tasks/:id/attempts` and opens the log streaming panel.

### 3. Settings Tab
- **Action: Toggle "Require Review"**
    - **Submission**: Calls `PUT /api/v1/projects/:id` with updated settings.
    - **Effect**: Persists whether agent changes should be automatically merged or require human approval.
- **Action: "Refresh Project"**
    - **Trigger**: Click refresh button.
    - **Effect**: Triggers `refetch()` on the `useProjectDetail` hook.
