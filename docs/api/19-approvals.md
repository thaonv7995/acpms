# Approvals API

API endpoints cho tool approval workflow (process-scoped).

## Base Path

`/api/v1/execution-processes/:id/approvals` và `/api/v1/approvals`

## Authentication

Yêu cầu JWT Bearer Token.

## Endpoints

### 1. GET `/api/v1/execution-processes/:id/approvals/pending`

Lấy danh sách approvals đang chờ phản hồi cho một execution process.

#### Path Parameters

- `id` (UUID, required): Execution process ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Pending approvals for execution process retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "attempt_id": "550e8400-e29b-41d4-a716-446655440001",
      "execution_process_id": "550e8400-e29b-41d4-a716-446655440002",
      "tool_use_id": "toolu_...",
      "tool_name": "bash",
      "tool_input": {},
      "status": "pending",
      "created_at": "2026-02-07T10:00:00Z"
    }
  ]
}
```

**Permissions**: Cần `ViewTask` trên project chứa execution process.

**Backend**: `crates/server/src/routes/approvals.rs::get_pending_approvals_for_process`

---

### 2. POST `/api/v1/approvals/:approval_ref/respond`

Gửi quyết định approve/deny cho approval request.

`approval_ref` hỗ trợ:
- `approval_id` (UUID) là định danh chuẩn.
- `tool_use_id` (legacy compatibility) vẫn được chấp nhận để phục vụ migration.

#### Path Parameters

- `approval_ref` (string, required): approval UUID hoặc legacy tool use ID

#### Request Body

```json
{
  "decision": "approve",
  "reason": "Looks safe"
}
```

**Fields**:
- `decision` (string, required): `approve` | `deny`
- `reason` (string, optional): lý do deny/approve

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Tool approved successfully"
}
```

**Other statuses**:
- `403 Forbidden`: thiếu quyền `ApproveTools`.
- `404 Not Found`: không tìm thấy approval.
- `409 Conflict`: approval không còn ở trạng thái `pending` (đã có client khác xử lý trước).

**Permissions**: Cần `ApproveTools` trên project chứa approval.

**Backend**: `crates/server/src/routes/approvals.rs::respond_to_approval`

---

### 3. GET `/api/v1/approvals/stream/ws`

WebSocket stream cho approvals realtime.

#### Query Parameters

- `projection` (optional): `legacy` (default) | `patch`
- `since_seq` (optional, number): cursor sequence để catch-up incremental
- `attempt_id` (optional, UUID): filter theo attempt
- `execution_process_id` (optional, UUID): filter theo execution process

#### Message Shapes

- `snapshot`: full snapshot approvals map + `sequence_id` (patch mode)
- `upsert`/`remove`: incremental patch operations (patch mode)
- `gap_detected`: báo cursor invalid/overflow, client cần resync snapshot

**Backend**: `crates/server/src/routes/websocket.rs::approvals_ws_handler`
