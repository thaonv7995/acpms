# Project Assistant API

API endpoints cho Project Assistant — interactive chat sessions với AI agent trong context của project.

## Base Path

`/api/v1/projects/:project_id/assistant`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. POST `/api/v1/projects/:project_id/assistant/sessions`

Tạo assistant session mới.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Request Body

```json
{
  "force_new": true
}
```

**Fields**:
- `force_new` (boolean, optional, default `true`): Nếu `true`, kết thúc session active (nếu có) và tạo mới. Nếu `false`, get hoặc create session (get_or_create).

**Permissions**: User phải có `ViewProject` permission.

#### Response

**Status**: `201 Created`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Session created",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "project_id": "...",
    "user_id": "...",
    "status": "active",
    "s3_log_key": null,
    "created_at": "2026-01-13T10:00:00Z",
    "ended_at": null
  }
}
```

**Note**: Khi `force_new = true`, session active hiện tại sẽ được archive (upload logs lên S3) và kết thúc.

#### Backend

`crates/server/src/routes/project_assistant.rs::create_session`

---

### 2. GET `/api/v1/projects/:project_id/assistant/sessions`

Lấy danh sách sessions.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Sessions retrieved",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "project_id": "...",
      "user_id": "...",
      "status": "active",
      "s3_log_key": null,
      "created_at": "2026-01-13T10:00:00Z",
      "ended_at": null
    }
  ]
}
```

**Note**: Chỉ trả về tối đa 3 sessions gần nhất. Chỉ trả về sessions của user hiện tại.

**Permissions**: User phải có `ViewProject` permission.

#### Backend

`crates/server/src/routes/project_assistant.rs::list_sessions`

---

### 3. GET `/api/v1/projects/:project_id/assistant/sessions/:session_id`

Lấy chi tiết session với messages.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Session retrieved",
  "data": {
    "session": {
      "id": "...",
      "project_id": "...",
      "user_id": "...",
      "status": "active",
      "s3_log_key": null,
      "created_at": "2026-01-13T10:00:00Z",
      "ended_at": null
    },
    "messages": [
      {
        "id": "...",
        "session_id": "...",
        "role": "assistant",
        "content": "Hello! I'm ready to help...",
        "metadata": null,
        "created_at": "2026-01-13T10:00:05Z"
      }
    ]
  }
}
```

**Session Isolation**: Chỉ owner của session mới có thể truy cập. Nếu `user_id` không khớp → `403 Forbidden`.

**Logs**: Messages được đọc từ local JSONL log file (hoặc S3 nếu session đã ended).

**Permissions**: User phải có `ViewProject` permission + session ownership.

#### Backend

`crates/server/src/routes/project_assistant.rs::get_session`

---

### 4. POST `/api/v1/projects/:project_id/assistant/sessions/:session_id/start`

Spawn agent CLI để bắt đầu session.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Request Body

Không có.

#### Response

**Status**: `202 Accepted`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Agent starting"
}
```

**Note**:
- Agent CLI được spawn qua worker pool với project instruction.
- Nếu agent đã đang chạy → trả `200 OK` với "Agent already running".
- Session phải có status `active`.
- Agent sẽ trả lời greeting qua stdout, orchestrator stream vào assistant log.

**Permissions**: User phải có `ViewProject` permission + session ownership.

#### Backend

`crates/server/src/routes/project_assistant.rs::start_session`

---

### 5. GET `/api/v1/projects/:project_id/assistant/sessions/:session_id/status`

Kiểm tra agent CLI có đang chạy không.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Session status",
  "data": {
    "active": true
  }
}
```

**Permissions**: User phải có `ViewProject` permission + session ownership.

#### Backend

`crates/server/src/routes/project_assistant.rs::get_session_status`

---

### 6. POST `/api/v1/projects/:project_id/assistant/sessions/:session_id/messages`

Gửi message mới (spawn agent mới).

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Request Body

```json
{
  "content": "Hãy tạo API endpoint cho user profile",
  "attachments": [
    {
      "key": "projects/<project_id>/assistant-attachments/<uuid>-file.txt",
      "filename": "file.txt"
    }
  ]
}
```

**Fields**:
- `content` (string, required): Nội dung message
- `attachments` (array, optional): Danh sách file attachments (max 5 files, max 1MB mỗi file, chỉ text/* và application/json)

#### Response

**Status**: `202 Accepted`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Message submitted"
}
```

**Note**:
- Nếu agent CLI đang chạy → trả `409 Conflict` với message "Use POST /input for follow-up messages".
- Dùng endpoint này khi agent chưa chạy (send message mới = spawn agent mới).
- Dùng `POST /input` khi agent đang chạy (follow-up).

**Permissions**: User phải có `ViewProject` permission + session ownership.

#### Backend

`crates/server/src/routes/project_assistant.rs::post_message`

---

### 7. POST `/api/v1/projects/:project_id/assistant/sessions/:session_id/input`

Gửi follow-up input cho agent đang chạy.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Request Body

```json
{
  "content": "Hãy thêm validation cho email field"
}
```

**Fields**:
- `content` (string, required): Follow-up message

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Input sent"
}
```

**Note**:
- Input được forward vào stdin của agent CLI đang chạy.
- Nếu không có agent đang chạy → `404 Not Found`.
- Message cũng được append vào JSONL log.

**Permissions**: User phải có `ViewProject` permission + session ownership.

#### Backend

`crates/server/src/routes/project_assistant.rs::post_input`

---

### 8. POST `/api/v1/projects/:project_id/assistant/sessions/:session_id/confirm-tool`

Xác nhận hoặc từ chối tool call từ agent.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Request Body

```json
{
  "tool_call_id": "call_abc123",
  "confirmed": true
}
```

**Fields**:
- `tool_call_id` (string, required): ID của tool call cần confirm
- `confirmed` (boolean, required): `true` = confirm, `false` = reject

#### Supported Tools

| Tool Name | Permission Required | Tạo entity |
|---|---|---|
| `create_requirement` | `CreateRequirement` | Tạo requirement mới |
| `create_task` | `CreateTask` | Tạo task mới |

#### Response (confirmed)

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Requirement created",
  "data": {
    "tool_call_id": "call_abc123",
    "confirmed": true,
    "entity_type": "requirement",
    "entity_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

#### Response (rejected)

```json
{
  "success": true,
  "code": "0000",
  "message": "Tool call rejected",
  "data": {
    "tool_call_id": "call_abc123",
    "confirmed": false
  }
}
```

**Idempotency**: Nếu tool call đã được xử lý → trả về kết quả cũ.

**Permissions**: Session ownership + tool-specific permissions.

#### Backend

`crates/server/src/routes/project_assistant.rs::confirm_tool`

---

### 9. POST `/api/v1/projects/:project_id/assistant/sessions/:session_id/end`

Kết thúc session.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Request Body

Không có.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Session ended",
  "data": {
    "id": "...",
    "project_id": "...",
    "user_id": "...",
    "status": "ended",
    "s3_log_key": "assistant-logs/...",
    "created_at": "2026-01-13T10:00:00Z",
    "ended_at": "2026-01-13T11:00:00Z"
  }
}
```

**Note**:
- Terminate agent CLI nếu đang chạy.
- Upload JSONL logs lên S3.
- Nếu session đã `ended` → trả về session hiện tại.

**Permissions**: User phải có `ViewProject` permission + session ownership.

#### Backend

`crates/server/src/routes/project_assistant.rs::end_session`

---

### 10. POST `/api/v1/projects/:project_id/assistant/attachments/upload-url`

Lấy presigned upload URL cho file attachment.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Request Body

```json
{
  "filename": "requirements.txt",
  "content_type": "text/plain"
}
```

**Fields**:
- `filename` (string, required): Tên file (sẽ được sanitize)
- `content_type` (string, required): MIME type

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Upload URL created",
  "data": {
    "upload_url": "https://s3.amazonaws.com/...",
    "key": "projects/<project_id>/assistant-attachments/<uuid>-requirements.txt"
  }
}
```

**Note**: Upload URL có hiệu lực 1 giờ.

**Permissions**: User phải có `ViewProject` permission.

#### Backend

`crates/server/src/routes/project_assistant.rs::get_assistant_attachment_upload_url`

---

### 11. WS `/api/v1/projects/:project_id/assistant/sessions/:session_id/logs/ws`

WebSocket endpoint cho real-time log streaming của assistant session.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

#### Connection

**URL**: `ws://localhost:3000/api/v1/projects/:project_id/assistant/sessions/:session_id/logs/ws`

**Protocol**: WebSocket

**Authentication**: JWT token qua `Sec-WebSocket-Protocol`

#### Message Format

**Outgoing**: Assistant log messages (JSONL format)
```json
{
  "id": "...",
  "session_id": "...",
  "role": "assistant",
  "content": "Response text...",
  "metadata": null,
  "created_at": "2026-01-13T10:00:00Z"
}
```

**Message Roles**: `assistant`, `user`, `system`, `stderr`

#### Backend

`crates/server/src/routes/websocket.rs::assistant_logs_ws_handler`

---

## Session Lifecycle

```
create_session → start → [messages/input ↔ logs/ws] → end
```

1. **Create**: `POST /sessions` (force_new=true kết thúc session cũ)
2. **Start**: `POST /sessions/:id/start` (spawn agent CLI)
3. **Interact**:
   - `POST /sessions/:id/messages` — gửi message mới (khi agent chưa chạy)
   - `POST /sessions/:id/input` — follow-up input (khi agent đang chạy)
   - `POST /sessions/:id/confirm-tool` — confirm/reject tool calls
   - `WS /sessions/:id/logs/ws` — stream logs real-time
4. **End**: `POST /sessions/:id/end` (terminate agent, archive logs)

## Session Status Values

- `active`: Session đang hoạt động
- `ended`: Session đã kết thúc

## Permissions

- Tất cả endpoints yêu cầu `ViewProject` permission
- `confirm-tool` yêu cầu thêm permission của tool cụ thể (`CreateRequirement`, `CreateTask`)
- **Session Isolation**: Chỉ owner (user tạo session) mới truy cập được session
