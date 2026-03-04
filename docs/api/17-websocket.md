# WebSocket API

API endpoints cho WebSocket connections và real-time streaming.

## Base Path

`/ws` (root level, không có `/api/v1` prefix)

## Authentication

Tất cả WebSocket connections yêu cầu JWT Bearer Token.

**Authentication Methods**:
1. `Sec-WebSocket-Protocol`: `acpms-bearer, <jwt_token>` (browser clients)
2. Authorization header: `Authorization: Bearer <token>` (non-browser clients)

---

## Endpoints

### 1. WS `/ws/attempts/:id/logs`

Real-time log streaming cho attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Connection

**URL**: `ws://localhost:3000/ws/attempts/:id/logs`

**Protocol**: WebSocket

**Authentication**: JWT token qua `Sec-WebSocket-Protocol` hoặc Authorization header

#### Message Format

**Incoming** (optional): client có thể gửi input để forward vào orchestrator
```json
{
  "type": "UserInput",
  "content": "Please continue with retry-safe implementation"
}
```

**Outgoing**:
```json
{
  "type": "Log",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "attempt_id": "...",
  "log_type": "stdout",
  "content": "Log message",
  "timestamp": "2026-01-13T10:00:00Z"
}
```

**Message Types**:
- `Log`
- `Status`
- `ApprovalRequest`
- `UserMessage`

#### Frontend Usage

**File**: `frontend/src/hooks/useAttemptLogs.ts`

**Example**:
```typescript
const ws = new WebSocket(
  `ws://localhost:3000/ws/attempts/${attemptId}/logs`,
  ['acpms-bearer', token]
);

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.type === 'log') {
    // Handle log entry
    addLogEntry(message.payload);
  }
};
```

**Màn hình**: 
- Task Detail Page - Logs Panel
- Project Tasks Page - ViewLogsModal

**Components**: `VirtualizedListWrapper.tsx`

**Backend**: `crates/server/src/routes/websocket.rs::ws_handler`

---

### 2. WS `/ws/attempts/:id/diffs`

Real-time diff streaming (alias của logs endpoint).

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Connection

**URL**: `ws://localhost:3000/ws/attempts/:id/diffs`

**Protocol**: WebSocket

**Message Format**: Giống như `/ws/attempts/:id/logs`

#### Frontend Usage

Giống như `/ws/attempts/:id/logs`

**Backend**: `crates/server/src/routes/websocket.rs::ws_handler`

---

### 3. WS `/ws/projects/:project_id/agents`

Real-time agent activity streaming cho project.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Connection

**URL**: `ws://localhost:3000/ws/projects/:project_id/agents`

**Protocol**: WebSocket

#### Message Format

**Outgoing**:
```json
{
  "event": {
    "type": "Status",
    "attempt_id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "running",
    "timestamp": "2026-01-13T10:00:00Z"
  },
  "task_id": "...",
  "task_title": "Task Title"
}
```

#### Frontend Usage

**File**: `frontend/src/hooks/useProjectAgentLogs.ts`

**Màn hình**: Project Tasks Page

**Component**: `LiveAgentActivity.tsx`

**Backend**: `crates/server/src/routes/websocket.rs::project_ws_handler`

---

### 4. GET `/api/v1/projects/:project_id/agents/active`

Lấy danh sách active agents của project.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "data": [
    {
      "attempt_id": "550e8400-e29b-41d4-a716-446655440000",
      "task_id": "...",
      "task_title": "Fix retry metadata",
      "task_type": "feature",
      "started_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/hooks/useProjectAgentLogs.ts`

**Màn hình**: Project Tasks Page

**Backend**: `crates/server/src/routes/websocket.rs::get_project_active_agents`

---

### 5. WS `/ws/agent-activity/status`

Real-time agent activity status updates (global dashboard).

#### Connection

**URL**: `ws://localhost:3000/ws/agent-activity/status`

#### Message Format

**Outgoing**: Agent status change events.

**Backend**: `crates/server/src/routes/websocket.rs::agent_activity_ws_handler`

---

### 6. WS `/api/v1/agent/auth/sessions/:id/ws`

Real-time auth session updates.

#### Path Parameters

- `id` (UUID, required): Auth session ID

#### Query Parameters

- `since_seq` (number, optional): Sequence number để catch-up

#### Message Types

- `snapshot`: Full session state
- `upsert`: Session update
- `gap_detected`: Sequence gap

**Backend**: `crates/server/src/routes/websocket.rs::agent_auth_session_ws_handler`

Xem chi tiết tại [22-agent-provider-auth.md](./22-agent-provider-auth.md).

---

### 7. WS `/api/v1/execution-processes/:id/raw-logs/ws`

WebSocket stream cho raw logs của execution process.

#### Path Parameters

- `id` (UUID, required): Execution process ID

**Backend**: `crates/server/src/routes/websocket.rs::execution_process_raw_logs_ws_handler`

---

### 8. WS `/api/v1/execution-processes/:id/normalized-logs/ws`

WebSocket stream cho normalized logs của execution process.

#### Path Parameters

- `id` (UUID, required): Execution process ID

**Backend**: `crates/server/src/routes/websocket.rs::execution_process_normalized_logs_ws_handler`

---

### 9. WS `/api/v1/execution-processes/stream/attempt/ws`

WebSocket stream cho execution processes theo attempt.

**Backend**: `crates/server/src/routes/websocket.rs::execution_processes_ws_handler`

---

### 10. WS `/api/v1/execution-processes/stream/session/ws`

WebSocket stream cho execution processes theo session.

**Backend**: `crates/server/src/routes/websocket.rs::execution_processes_session_ws_handler`

---

### 11. WS `/api/v1/projects/:project_id/assistant/sessions/:session_id/logs/ws`

WebSocket stream cho Project Assistant session logs.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `session_id` (UUID, required): Session ID

**Backend**: `crates/server/src/routes/websocket.rs::assistant_logs_ws_handler`

Xem chi tiết tại [21-project-assistant.md](./21-project-assistant.md).

---

### 12. WS `/api/v1/attempts/:id/stream/ws`

WebSocket stream cho attempt (Phase H — Vibe Kanban parity).

#### Path Parameters

- `id` (UUID, required): Attempt ID

**Backend**: `crates/server/src/routes/websocket.rs::attempt_stream_ws_handler`

---

### 13. WS `/api/v1/approvals/stream/ws`

WebSocket stream cho tool approval events (SDK mode).

**Backend**: `crates/server/src/routes/websocket.rs::approvals_ws_handler`

---

## WebSocket Protocol

### Connection Flow

1. **Connect**: Client connects với JWT token
2. **Authenticate**: Server verifies token và checks permissions
3. **Subscribe**: Server subscribes client vào broadcast channel
4. **Stream**: Server streams events matching filters
5. **Disconnect**: Client disconnects hoặc server closes connection

### Error Handling

**Connection Errors**:
- `401 Unauthorized`: Invalid or missing token
- `403 Forbidden`: No permission to view resource
- `404 Not Found`: Resource not found

**Message Errors**: 
- Invalid JSON: Connection closed
- Protocol errors: Connection closed

### Reconnection

Frontend nên implement auto-reconnection logic:
- Exponential backoff
- Max retry attempts
- Re-authenticate on reconnect

## Permissions

- **Attempt Logs WebSocket**: User phải có `ViewProject` permission
- **Project Agents WebSocket**: User phải có `ViewProject` permission
- **Get Active Agents**: User phải có `ViewProject` permission

## Performance Notes

- WebSocket connections được manage qua broadcast channels
- Events được filtered by attempt_id hoặc project_id
- Multiple clients có thể subscribe cùng một resource
- Server tự động cleanup khi client disconnect
