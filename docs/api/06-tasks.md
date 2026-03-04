# Tasks API

API endpoints cho quản lý tasks.

## Base Path

`/api/v1/tasks`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. GET `/api/v1/tasks`

Lấy danh sách tasks của project.

#### Query Parameters

- `project_id` (UUID, required): Project ID
- `sprint_id` (UUID, optional): Filter theo sprint

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Tasks retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "project_id": "...",
      "requirement_id": null,
      "sprint_id": null,
      "title": "Task Title",
      "description": "Task description",
      "task_type": "feature",
      "status": "todo",
      "assigned_to": null,
      "parent_task_id": null,
      "gitlab_issue_id": null,
      "metadata": {},
      "created_by": "...",
      "created_at": "2026-01-13T10:00:00Z",
      "updated_at": "2026-01-13T10:00:00Z",
      "latest_attempt_id": null,
      "has_in_progress_attempt": false,
      "last_attempt_failed": false,
      "executor": null
    }
  ]
}
```

**Note**: Includes attempt status fields for kanban display.

#### Frontend Usage

**File**: `frontend/src/api/tasks.ts`

**Màn hình**: 
- Project Tasks Page (`/projects/:id/tasks`)
- Task Board Page (`/tasks/board`)

**Components**: `TaskListTab.tsx`, `KanbanTab.tsx`

**Backend**: `crates/server/src/routes/tasks.rs::list_tasks`

---

### 2. POST `/api/v1/tasks`

Tạo task mới.

#### Request Body

```json
{
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "requirement_id": null,
  "sprint_id": null,
  "title": "Task Title",
  "description": "Task description",
  "task_type": "feature",
  "parent_task_id": null
}
```

**Fields**:
- `project_id` (UUID, required): Project ID
- `requirement_id` (UUID, optional): Requirement ID
- `sprint_id` (UUID, optional): Sprint ID
- `title` (string, required): 1-200 characters
- `description` (string, optional): Task description
- `task_type` (string, required): "feature" | "bug" | "refactor" | "docs" | "test" | "init"
- `parent_task_id` (UUID, optional): Parent task ID (for subtasks)

**Permissions**: User phải có `CreateTask` permission.

#### Response

**Status**: `201 Created`

**Body**: TaskDto object

#### Frontend Usage

**File**: `frontend/src/components/modals/CreateTaskModal.tsx`

**Màn hình**: 
- Dashboard Page (từ modal)
- Project Tasks Page (từ modal)

**Backend**: `crates/server/src/routes/tasks.rs::create_task`

---

### 3. GET `/api/v1/tasks/:id`

Lấy thông tin task theo ID.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Response

**Status**: `200 OK`

**Body**: TaskDto object

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page (`/projects/:projectId/task/:taskId`)

**Backend**: `crates/server/src/routes/tasks.rs::get_task`

---

### 4. PUT `/api/v1/tasks/:id`

Cập nhật task.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Request Body

```json
{
  "title": "New Title",
  "description": "New description",
  "task_type": "bug"
}
```

**Fields** (all optional):
- `title` (string): 1-200 characters
- `description` (string)
- `task_type` (string)

**Permissions**: User phải có `ModifyTask` permission.

#### Response

**Status**: `200 OK`

**Body**: TaskDto object

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/tasks.rs::update_task`

---

### 5. DELETE `/api/v1/tasks/:id`

Xóa task.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Task deleted successfully"
}
```

**Permissions**: User phải có `DeleteTask` permission.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/tasks.rs::delete_task`

---

### 6. PUT `/api/v1/tasks/:id/status`

Cập nhật status của task.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Request Body

```json
{
  "status": "in_progress"
}
```

**Fields**:
- `status` (string, required): "todo" | "in_progress" | "in_review" | "blocked" | "done" | "archived"

#### Response

**Status**: `200 OK`

**Body**: TaskDto object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectTasksPage.tsx`

**Màn hình**: Project Tasks Page - Kanban Tab

**Nhiệm vụ**: Drag & drop task giữa các columns

**Backend**: `crates/server/src/routes/tasks.rs::update_task_status`

---

### 7. GET `/api/v1/tasks/:id/children`

Lấy danh sách subtasks.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Task children retrieved successfully",
  "data": [
    { ... }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/tasks.rs::get_task_children`

---

### 8. POST `/api/v1/tasks/:id/assign`

Assign task cho user.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Request Body

```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Fields**:
- `user_id` (UUID | null, required): User ID hoặc null để unassign

#### Response

**Status**: `200 OK`

**Body**: TaskDto object

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/tasks.rs::assign_task`

---

### 9. PUT `/api/v1/tasks/:id/metadata`

Cập nhật metadata của task.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Request Body

```json
{
  "metadata": {
    "priority": "high",
    "labels": ["bug", "urgent"],
    "estimated_hours": 8
  }
}
```

**Fields**:
- `metadata` (object, required): JSON metadata

#### Response

**Status**: `200 OK`

**Body**: TaskDto object

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/tasks.rs::update_task_metadata`

---

## Task Types

Valid task types:
- `feature`: New feature
- `bug`: Bug fix
- `refactor`: Code refactoring
- `docs`: Documentation
- `test`: Testing
- `init`: Initialization task (special type)

## Task Status

Valid statuses:
- `todo`: Not started
- `in_progress`: In progress
- `in_review`: Waiting for review
- `blocked`: Blocked by dependency
- `done`: Completed
- `archived`: Archived

## Permissions

- **List Tasks**: User phải có `ViewProject` permission
- **Create Task**: User phải có `CreateTask` permission
- **Get Task**: User phải có `ViewProject` permission
- **Update Task**: User phải có `ModifyTask` permission
- **Delete Task**: User phải có `DeleteTask` permission
- **Update Status**: User phải có `ModifyTask` permission
- **Assign Task**: User phải có `ModifyTask` permission
- **Update Metadata**: User phải có `ModifyTask` permission
