# SRS: Project Detail

## 1. Introduction
The Project Detail screen provides a deep-dive into a single project's health, task progress, and configuration. It acts as the primary cockpit for project-specific operations.

## 2. Access Control
- **Roles**: All authenticated users with project access.
- **Permissions**: 
  - `admin` / `PO`: Can edit all settings and manage tasks.
  - `developer` / `QA`: Can view all and manage tasks assigned to them.

## 3. UI Components
- **Stats Dashboard**: Project-specific metrics (Success rate, active tasks, sprint progress).
- **Navigation Tabs**: Summary, Tasks (Kanban), Requirements, Architecture, Settings.
- **Active Sprint Panel**: Current sprint goals and burn-down.

## 4. Functional Requirements

### [SRS-PRD-001] Load Project Context
- **Trigger**: Screen navigation to `/projects/:id`.
- **Input**: `project_id`.
- **Output**: Populated project header and "Summary" tab.
- **System Logic**: 
  - `GET /api/v1/projects/:id` (Details)
  - `GET /api/v1/tasks?project_id=:id` (Task-based metrics ở tầng client)

### [SRS-PRD-002] Manage Requirements
- **Trigger**: Navigation to "Requirements" tab -> Add/Edit.
- **Input**: Requirement text, priority, category.
- **Output**: Success toast; updated requirements list.
- **System Logic**: Calls `POST/PUT /api/v1/projects/:id/requirements`.
- **Validation**: Requirements must be parsed and indexed for agent context.

### [SRS-PRD-003] Update Project Settings
- **Trigger**: "Settings" tab -> Submit changes.
- **Input**: `branch_pattern`, `review_workflow_type`, `deployment_target`.
- **Output**: Success toast.
- **System Logic**: Calls `PUT /api/v1/projects/:id`.

### [SRS-PRD-004] Sync Repository
- **Trigger**: Click "Sync with Git" button.
- **Input**: None.
- **Output**: Loading state -> "Last sync: Just now".
- **System Logic**: Endpoint sync repository riêng chưa có trong backend hiện tại; UI hiện chủ yếu dùng `refetch` dữ liệu project/tasks.

### [SRS-PRD-005] Tab Switching
- **Trigger**: Click tab header.
- **Input**: Tab ID.
- **Output**: Dynamic component swap without page reload.
- **System Logic**: Managed via `react-router` nested routes.

## 5. Non-Functional Requirements
- **Consistency**: The Kanban board in the "Tasks" tab must be identical in behavior to the global Kanban view.
- **Data Freshness**: System must poll for stat updates every 60s while the page is active.
