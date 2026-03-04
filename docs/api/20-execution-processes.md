# Execution Processes API

API endpoints cho process-centric execution flow.

## Base Path

`/api/v1/execution-processes`

## Authentication

Yêu cầu JWT Bearer Token.

## Endpoints

### 1. GET `/api/v1/execution-processes?attempt_id=:attempt_id`

Lấy danh sách execution process của một attempt, sắp theo `created_at ASC, id ASC`.

#### Query Parameters

- `attempt_id` (UUID, required): attempt ID cần truy vấn process chain.

#### Permissions

- Cần `ViewProject` trên project chứa attempt.

---

### 2. GET `/api/v1/execution-processes/:id`

Lấy chi tiết một execution process.

#### Path Parameters

- `id` (UUID, required): execution process ID.

#### Permissions

- Cần `ViewProject`.

---

### 3. POST `/api/v1/execution-processes/:id/follow-up`

Tạo follow-up run từ một execution process nguồn.

#### Path Parameters

- `id` (UUID, required): source execution process ID.

#### Request Body

```json
{
  "prompt": "Continue from latest state"
}
```

#### Permissions

- Cần `ExecuteTask`.

#### Notes

- API này adapter về flow process-first (tạo process mới, không mutate process cũ).
- Backend sẽ tự gán `source_execution_process_id` từ path `:id`.

---

### 4. POST `/api/v1/execution-processes/:id/reset`

Reset worktree checkpoint theo execution process.

#### Path Parameters

- `id` (UUID, required): execution process ID.

#### Request Body

```json
{
  "perform_git_reset": true,
  "force_when_dirty": false
}
```

#### Fields

- `perform_git_reset` (boolean, optional, default `false`): bật thao tác `git reset --hard HEAD`.
- `force_when_dirty` (boolean, optional, default `false`): cho phép reset khi worktree có uncommitted changes.

#### Response

```json
{
  "success": true,
  "code": "0000",
  "message": "Execution process reset successfully",
  "data": {
    "process_id": "550e8400-e29b-41d4-a716-446655440000",
    "worktree_path": "/tmp/worktree-xyz",
    "git_reset_applied": true,
    "worktree_was_dirty": true,
    "force_when_dirty": true,
    "requested_by_user_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "requested_at": "2026-02-27T10:45:00Z"
  }
}
```

- `force_when_dirty` (boolean): phản ánh cờ force từ request để audit.
- `requested_by_user_id` (UUID): user thực hiện reset.
- `requested_at` (RFC3339 timestamp): thời điểm backend xử lý request reset.

#### Error Cases

- `400 Bad Request`:
  - process không có worktree path,
  - worktree path không tồn tại,
  - worktree dirty nhưng chưa bật `force_when_dirty`.
- `403 Forbidden`: thiếu quyền `ExecuteTask`.
- `404 Not Found`: execution process không tồn tại.

#### Permissions

- Cần `ExecuteTask`.

---

### 5. GET `/api/v1/execution-processes/:id/raw-logs`

Lấy raw logs trong window của execution process.

### 6. GET `/api/v1/execution-processes/:id/normalized-logs`

Lấy normalized logs trong window của execution process.

#### Permissions

- Hai endpoint logs cần `ViewProject`.

---

### 7. WS `/api/v1/execution-processes/:id/raw-logs/ws`

WebSocket stream cho raw logs (real-time).

**Backend**: `crates/server/src/routes/websocket.rs::execution_process_raw_logs_ws_handler`

---

### 8. WS `/api/v1/execution-processes/:id/normalized-logs/ws`

WebSocket stream cho normalized logs (real-time).

**Backend**: `crates/server/src/routes/websocket.rs::execution_process_normalized_logs_ws_handler`

---

### 9. WS `/api/v1/execution-processes/stream/attempt/ws`

WebSocket stream — nhận events cho tất cả execution processes thuộc một attempt.

**Backend**: `crates/server/src/routes/websocket.rs::execution_processes_ws_handler`

---

### 10. WS `/api/v1/execution-processes/stream/session/ws`

WebSocket stream — nhận events cho tất cả execution processes thuộc một assistant session.

**Backend**: `crates/server/src/routes/websocket.rs::execution_processes_session_ws_handler`

---

### 11. GET `/api/v1/execution-processes/:id/approvals/pending`

Lấy pending tool approvals cho execution process (SDK mode).

#### Path Parameters

- `id` (UUID, required): execution process ID.

**Backend**: `crates/server/src/routes/approvals.rs::get_pending_approvals_for_process`
