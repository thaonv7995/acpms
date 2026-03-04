# Agent Provider Auth API

API endpoints cho agent provider authentication — quản lý auth sessions cho các CLI providers (Claude Code, OpenAI Codex, Gemini CLI, Cursor).

## Base Path

`/api/v1/agent`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

## Feature Flag

- Server-side: `AGENT_UI_AUTH_ENABLED` (default: enabled)
- Disable values: `0`, `false`, `off`, `no`

---

## Endpoints

### 1. GET `/api/v1/agent/providers/status`

Lấy status của tất cả supported providers.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Provider statuses retrieved",
  "data": {
    "providers": [
      {
        "provider": "claude-code",
        "available": true,
        "availability_reason": "ok",
        "auth_state": "authenticated",
        "cli_path": "/usr/local/bin/claude",
        "display_name": "Claude Code"
      },
      {
        "provider": "openai-codex",
        "available": false,
        "availability_reason": "cli_missing",
        "auth_state": "unknown",
        "cli_path": null,
        "display_name": "OpenAI Codex"
      }
    ]
  }
}
```

**Provider Values**: `claude-code`, `openai-codex`, `gemini-cli`, `cursor`

**Auth State Values**: `authenticated`, `unauthenticated`, `expired`, `unknown`

**Availability Reason Values**: `ok`, `cli_missing`, `not_authenticated`, `auth_expired`, `auth_check_failed`

#### Backend

`crates/server/src/routes/agent.rs::get_provider_statuses`

---

### 2. POST `/api/v1/agent/auth/initiate`

Khởi tạo auth session cho một provider.

#### Request Body

```json
{
  "provider": "gemini-cli"
}
```

**Fields**:
- `provider` (string, required): Provider name (`claude-code`, `openai-codex`, `gemini-cli`, `cursor`)

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Auth session initiated",
  "data": {
    "session_id": "550e8400-e29b-41d4-a716-446655440000",
    "provider": "gemini-cli",
    "flow_type": "oob",
    "status": "initiated",
    "action_url": "https://accounts.google.com/...",
    "action_code": null,
    "action_hint": "Open this URL and paste the code",
    "created_at": "2026-01-13T10:00:00Z",
    "expires_at": "2026-01-13T10:05:00Z"
  }
}
```

**Flow Types**:
- `oob` (Out-of-Band): User cần mở URL và paste code lại (Gemini CLI)
- `device_flow`: Device code flow (OpenAI Codex)
- `loopback_proxy`: OAuth redirect qua localhost proxy (Claude Code)

**Session Fields**:
- `action_url`: URL để user mở (nếu có)
- `action_code`: Device code để user nhập (nếu có)
- `action_hint`: Hướng dẫn cho user
- `allowed_loopback_port`: Port cho localhost callback (nếu có)

**Note**:
- Session sẽ tự động expire sau 5 phút.
- Agent CLI được spawn as child process, stdout parsed để extract URLs/codes.
- Session ownership check trên tất cả endpoints.

#### Backend

`crates/server/src/routes/agent.rs::initiate_agent_auth`

---

### 3. POST `/api/v1/agent/auth/submit-code`

Submit OOB code hoặc callback URL cho auth session.

#### Request Body

```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "code": "4/0AeaY..."
}
```

**Fields**:
- `session_id` (UUID, required): Auth session ID
- `code` (string, required): Auth code hoặc full localhost callback URL

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Code submitted",
  "data": {
    "session_id": "...",
    "status": "verifying",
    "accepted": true
  }
}
```

**Security**:
- Session ownership check (user phải là người tạo session).
- Rate limit per session window.
- Strict localhost callback validation (`127.0.0.1` / `localhost` only).
- Callback port validation against session loopback port.
- Non-localhost HTTP/HTTPS URLs bị reject.
- Callback proxy dùng `reqwest` với redirect following disabled.

#### Backend

`crates/server/src/routes/agent.rs::submit_agent_auth_code`

---

### 4. POST `/api/v1/agent/auth/cancel`

Cancel auth session.

#### Request Body

```json
{
  "session_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Fields**:
- `session_id` (UUID, required): Auth session ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Auth session cancelled",
  "data": {
    "session_id": "...",
    "status": "cancelled",
    "provider": "gemini-cli"
  }
}
```

**Note**: Backend sẽ kill child process nếu đang chạy.

#### Backend

`crates/server/src/routes/agent.rs::cancel_agent_auth`

---

### 5. GET `/api/v1/agent/auth/sessions/:id`

Lấy thông tin auth session.

#### Path Parameters

- `id` (UUID, required): Auth session ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Auth session retrieved",
  "data": {
    "session_id": "...",
    "provider": "gemini-cli",
    "flow_type": "oob",
    "status": "authenticated",
    "action_url": null,
    "action_code": null,
    "action_hint": null,
    "process_pid": 12345,
    "allowed_loopback_port": null,
    "last_error": null,
    "result": null,
    "created_at": "2026-01-13T10:00:00Z",
    "updated_at": "2026-01-13T10:01:00Z",
    "expires_at": "2026-01-13T10:05:00Z"
  }
}
```

**Session ownership**: Chỉ user tạo session mới truy cập được.

#### Backend

`crates/server/src/routes/agent.rs::get_agent_auth_session`

---

### 6. WS `/api/v1/agent/auth/sessions/:id/ws`

WebSocket endpoint cho real-time auth session updates.

#### Path Parameters

- `id` (UUID, required): Auth session ID

#### Query Parameters

- `since_seq` (number, optional): Sequence number để catch-up

#### Connection

**URL**: `ws://localhost:3000/api/v1/agent/auth/sessions/:id/ws?since_seq=0`

**Authentication**: JWT via `Sec-WebSocket-Protocol`

#### Message Types

**`snapshot`**: Full session state
```json
{
  "type": "snapshot",
  "data": {
    "sequence_id": 5,
    "session": { ... }
  }
}
```

**`upsert`**: Session update
```json
{
  "type": "upsert",
  "data": {
    "sequence_id": 6,
    "session": { ... }
  }
}
```

**`gap_detected`**: Sequence gap (client cần refetch)
```json
{
  "type": "gap_detected",
  "data": {
    "requested_since_seq": 2,
    "max_available_sequence_id": 10
  }
}
```

#### Backend

`crates/server/src/routes/websocket.rs::agent_auth_session_ws_handler`

---

## Auth Session Status Flow

```
initiated → action_required → verifying → authenticated
                                        → failed
                                        → cancelled
                                        → expired
```

## Security

1. **Redaction**: Raw codes và tokens MUST be masked in stdout logging.
2. **Session Isolation**: `submit-code` verifies session ownership.
3. **Timeouts**: Hard-kill child process after 5 minutes.
4. **Cancellation**: WebSocket disconnect / modal close → backend abort + kill child process.

## Permissions

- Tất cả endpoints yêu cầu authenticated user (JWT).
- Session ownership check trên tất cả session-specific endpoints.
