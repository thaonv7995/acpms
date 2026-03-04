# Task Attempts API

API endpoints cho task execution và attempts.

## Base Path

`/api/v1/tasks/:task_id/attempts` và `/api/v1/attempts/:id`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. GET `/api/v1/tasks/:task_id/attempts`

Lấy danh sách attempts của task.

#### Path Parameters

- `task_id` (UUID, required): Task ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Task attempts retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "task_id": "...",
      "status": "running",
      "started_at": "2026-01-13T10:00:00Z",
      "completed_at": null,
      "error_message": null,
      "metadata": {},
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/api/taskAttempts.ts`

**Màn hình**: 
- Task Detail Page
- Project Tasks Page

**Backend**: `crates/server/src/routes/task_attempts.rs::get_task_attempts`

---

### 2. POST `/api/v1/tasks/:task_id/attempts`

Tạo attempt mới cho task.

#### Path Parameters

- `task_id` (UUID, required): Task ID

#### Request Body

Không có (empty object `{}`)

#### Response

**Status**: `201 Created`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Task attempt created successfully",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "task_id": "...",
    "status": "queued",
    "created_at": "2026-01-13T10:00:00Z"
  }
}
```

**Note**: 
- Tự động update task status thành `in_progress`
- Submit job vào worker pool để execution
- Init tasks được execute trực tiếp (không qua worker pool)

**Permissions**: User phải có `ExecuteTask` permission.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::create_task_attempt`

---

### 3. GET `/api/v1/attempts/:id`

Lấy thông tin attempt theo ID.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**: TaskAttemptDto object

#### Frontend Usage

**File**: `frontend/src/api/taskAttempts.ts`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::get_attempt`

---

### 4. GET `/api/v1/attempts/:id/logs`

Lấy logs của attempt.

#### Rule (Backend Work Cap)

`limit` và `before` phải cap backend work. Không parse full JSONL/S3 object rồi mới áp pagination. Dùng tail read (most recent) hoặc head read (for `before` cursor). Cả local và S3 fallback đều áp dụng pagination. Cost O(page_size), không O(full_file_size).

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Attempt logs retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "attempt_id": "...",
      "type": "system",
      "message": "Starting task execution...",
      "timestamp": "2026-01-13T10:00:00Z",
      "level": "info"
    }
  ]
}
```

**Log Types**:
- `system`: System messages
- `stdout`: Standard output
- `stderr`: Standard error

#### Frontend Usage

**File**: `frontend/src/hooks/useAttemptLogs.ts`

**Màn hình**: 
- Task Detail Page - Logs Panel
- Project Tasks Page - ViewLogsModal

**Components**: `ViewLogsModal.tsx`, `VirtualizedListWrapper.tsx`

**Backend**: `crates/server/src/routes/task_attempts.rs::get_attempt_logs`

---

### 5. POST `/api/v1/attempts/:id/input`

Gửi input/follow-up message cho attempt đang chạy.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "input": "Please add error handling"
}
```

**Fields**:
- `input` (string, required): Follow-up message

**Permissions**: User phải có `ExecuteTask` permission.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Input sent successfully"
}
```

#### Frontend Usage

**File**: `frontend/src/components/tasks-page/TaskFollowUpSection.tsx`

**Màn hình**: 
- Task Detail Page
- Project Tasks Page - ViewLogsModal

**Backend**: `crates/server/src/routes/task_attempts.rs::send_attempt_input`

---

### 6. POST `/api/v1/attempts/:id/cancel`

Cancel attempt đang chạy.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "reason": "Cancelled by user",
  "force": false
}
```

**Fields**:
- `reason` (string, optional): Cancellation reason
- `force` (boolean, optional): Force kill after graceful timeout

**Note**: Chỉ có thể cancel attempts với status `queued` hoặc `running`.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Attempt cancelled: Cancelled by user"
}
```

**Note**: Task status sẽ được reset về `todo` để có thể retry.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::cancel_attempt`

---

### 7. POST `/api/v1/attempts/:id/retry`

Retry attempt đã failed.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

Không có (empty object `{}`)

#### Response

**Status**: `201 Created`

**Body**: RetryResponseDto (new attempt + retry_info)

**Note**: Tạo attempt mới với retry metadata.

#### Frontend Usage

**File**: `frontend/src/components/task-detail-page/AttemptRetryInfo.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::retry_attempt`

---

### 8. GET `/api/v1/attempts/:id/retry-info`

Lấy thông tin retry của attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Retry info retrieved successfully",
  "data": {
    "retry_count": 1,
    "max_retries": 3,
    "remaining_retries": 2,
    "can_retry": true,
    "auto_retry_enabled": false,
    "previous_attempt_id": "...",
    "previous_error": "...",
    "next_retry_attempt_id": null,
    "next_backoff_seconds": null
  }
}
```

#### Frontend Usage

**File**: `frontend/src/components/task-detail-page/AttemptRetryInfo.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::get_retry_info`

---

### 9. GET `/api/v1/attempts/:id/diff`

Lấy file diffs của attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Attempt diff retrieved successfully",
  "data": {
    "files": [
      {
        "change": "modified",
        "old_path": "src/file.ts",
        "new_path": "src/file.ts",
        "old_content": "...",
        "new_content": "...",
        "additions": 10,
        "deletions": 5
      }
    ],
    "total_files": 1,
    "total_additions": 10,
    "total_deletions": 5
  }
}
```

**Change Types**: "added", "deleted", "modified", "renamed"

#### Frontend Usage

**File**: `frontend/src/components/diff-viewer/DiffViewer.tsx`

**Màn hình**: 
- Task Detail Page - Diffs Tab
- Project Tasks Page - ViewLogsModal (Diffs mode)

**Backend**: `crates/server/src/routes/task_attempts.rs::get_attempt_diff`

---

### 10. GET `/api/v1/attempts/:id/diffs`

Alias của `/api/v1/attempts/:id/diff` (frontend compatibility).

#### Frontend Usage

Giống như `/api/v1/attempts/:id/diff`

---

### 11. GET `/api/v1/attempts/:id/branch-status`

Lấy branch status của attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Branch status retrieved successfully",
  "data": {
    "branch": "feature/task-123",
    "status": "ahead",
    "ahead_by": 5,
    "behind_by": 0
  }
}
```

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::get_branch_status`

---

### 12. POST `/api/v1/attempts/:id/approve`

Approve attempt và merge changes.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "commit_message": "Merge changes from task"
}
```

**Fields**:
- `commit_message` (string, optional): Custom commit message

**Permissions**: User phải có `ApproveAttempt` permission.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Attempt approved and merged successfully"
}
```

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::approve_attempt`

---

### 13. POST `/api/v1/attempts/:id/reject`

Reject attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "reason": "Changes don't meet requirements"
}
```

**Fields**:
- `reason` (string, optional): Rejection reason

**Permissions**: User phải có `ApproveAttempt` permission.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Attempt rejected successfully"
}
```

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/task_attempts.rs::reject_attempt`

---

### 14. POST `/api/v1/attempts/:id/resume` (Decommissioned Legacy Endpoint)

Endpoint tương thích cũ cho follow-up đã bị gỡ khỏi router primary API surface.

**Preferred**: dùng `POST /api/v1/execution-processes/:id/follow-up`.

**Availability**: endpoint này hiện không còn expose trong router (`404 Not Found`).

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "prompt": "Please continue with API error handling"
}
```

**Optional fields**:
- `source_execution_process_id` (UUID, optional): process nguồn để resume theo process context.

#### Response

**Status**: `200 OK`

**Body**: TaskAttemptDto object

**Backend**: `crates/server/src/routes/task_attempts.rs::resume_attempt`

---

### 15. GET `/api/v1/attempts/:id/structured-logs`

Lấy normalized/structured logs cho timeline UI.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Query Parameters

- `page` (number, optional): default `1`
- `page_size` (number, optional): default `100`, max `500`
- `include_subagents` (boolean, optional)
- `entry_types` (string, optional, comma-separated)
- `tool_names` (string, optional, comma-separated)

#### Response

**Status**: `200 OK`

**Body**: `StructuredLogsResponse` gồm `entries`, pagination info, và `file_diffs`.

**Backend**: `crates/server/src/routes/task_attempts.rs::get_structured_logs`

**Rule (Pagination)**: Pagination phải thực hiện phía server (SQL `LIMIT`/`OFFSET`). Không load toàn bộ entries rồi filter/paginate trong memory. Cost phải là O(page_size) mỗi request, không phải O(total_entries).

---

### 16. GET `/api/v1/attempts/:id/subagent-tree`

Lấy cây subagent của attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**: `SubagentTreeResponse` gồm danh sách node + tổng số subagent.

**Backend**: `crates/server/src/routes/task_attempts.rs::get_subagent_tree`

---

### 17. GET `/api/v1/attempts/:id/stream`

SSE stream (JSON Patch) cho task attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Query Parameters

- `since` (number, optional): sequence number để catch-up

#### Authentication

Yêu cầu header `Authorization: Bearer <token>`.

**Backend**: `crates/server/src/routes/streams.rs::stream_attempt_sse`

---

### 18. GET `/api/v1/attempts/:id/logs/ws`

Alias WebSocket endpoint dưới namespace `/api/v1`.

#### Path Parameters

- `id` (UUID, required): Attempt ID

**Backend**: `crates/server/src/routes/websocket.rs::ws_handler`

---

### 19. POST `/api/v1/tasks/:task_id/attempts/from-edit`

Tạo attempt mới từ manual edit (không spawn agent, chỉ tạo record).

#### Path Parameters

- `task_id` (UUID, required): Task ID

**Permissions**: `ExecuteTask`.

**Backend**: `crates/server/src/routes/task_attempts.rs::create_task_attempt_from_edit`

---

### 20. GET `/api/v1/attempts/:id/skills`

Lấy resolved skill chain của attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**: Danh sách skills đã sử dụng.

**Backend**: `crates/server/src/routes/task_attempts.rs::get_attempt_skills`

---

### 21. PATCH `/api/v1/attempts/:id/logs/:log_id`

Patch một log entry (ví dụ: cập nhật metadata).

#### Path Parameters

- `id` (UUID, required): Attempt ID
- `log_id` (UUID, required): Log entry ID

**Backend**: `crates/server/src/routes/task_attempts.rs::patch_attempt_log`

---

### 22. GET `/api/v1/attempts/:id/processes`

Lấy danh sách execution processes của attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**: Danh sách `AttemptExecutionProcessDto`.

**Backend**: `crates/server/src/routes/task_attempts.rs::get_attempt_execution_processes`

---

### 23. POST `/api/v1/attempts/:id/rebase`

Rebase attempt branch lên latest main.

#### Path Parameters

- `id` (UUID, required): Attempt ID

**Backend**: `crates/server/src/routes/task_attempts.rs::rebase_attempt`

---

### 24. GET `/api/v1/attempts/:id/diff-summary`

Lấy summary of diffs (số files, additions, deletions) mà không load full content.

#### Path Parameters

- `id` (UUID, required): Attempt ID

**Backend**: `crates/server/src/routes/task_attempts.rs::get_attempt_diff_summary`

---

### 25. WS `/api/v1/attempts/:id/stream/ws`

WebSocket stream cho attempt (Phase H — Vibe Kanban parity).

#### Path Parameters

- `id` (UUID, required): Attempt ID

**Backend**: `crates/server/src/routes/websocket.rs::attempt_stream_ws_handler`

---

## Attempt Status

Valid statuses:
- `queued`: Waiting in queue
- `running`: Currently executing
- `success`: Completed successfully
- `failed`: Failed with error
- `cancelled`: Cancelled by user

## Permissions

Các endpoint approvals liên quan attempt được tách ra tài liệu riêng tại `19-approvals.md`.

- **List Attempts**: User phải có `ViewProject` permission
- **Create Attempt**: User phải có `ExecuteTask` permission
- **Get Attempt**: User phải có `ViewProject` permission
- **Get Logs**: User phải có `ViewProject` permission
- **Send Input**: User phải có `ExecuteTask` permission
- **Cancel Attempt**: User phải có `ExecuteTask` permission
- **Retry Attempt**: User phải có `ExecuteTask` permission
- **Get Diff**: User phải có `ViewProject` permission
- **Approve/Reject**: User phải có `ApproveAttempt` permission
- **Rebase**: User phải có `ExecuteTask` permission
- **Get Skills**: User phải có `ViewProject` permission
- **Get Processes**: User phải có `ViewProject` permission
