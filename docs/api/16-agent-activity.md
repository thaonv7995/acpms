# Agent Activity API

API endpoints cho agent status và logs.

## Base Path

`/api/v1/agent-activity` và `/api/v1/agent`

## Authentication

`/agent-activity/*` dùng `AuthUser` extractor.  
`GET /api/v1/agent/status` cũng dùng `AuthUser` (JWT bearer).

---

## Endpoints

### 1. GET `/api/v1/agent-activity/status`

Lấy agent status tổng hợp.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Agent statuses retrieved",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Agent-1",
      "task_title": "Task Title",
      "project_name": "Project Name",
      "status": "running",
      "started_at": "2026-01-13T10:00:00Z",
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

**Note**: 
- Returns running attempts
- Includes queued attempts từ 1 giờ trước
- Includes completed/failed attempts từ 1 giờ trước
- Sorted by status priority (running > queued > others)

#### Frontend Usage

**File**: `frontend/src/pages/DashboardPage.tsx`

**Màn hình**: Dashboard Page

**Backend**: `crates/server/src/routes/agent_activity.rs::get_agent_status`

---

### 2. GET `/api/v1/agent-activity/logs`

Lấy agent logs tổng hợp.

#### Query Parameters

- `attempt_id` (UUID, optional): Filter theo attempt ID
- `project_id` (UUID, optional): Filter theo project ID
- `limit` (number, optional): Max logs to return (default: 100, max: 500)

#### Rule (Backend Work Cap)

`limit` phải cap cả response **và** backend work. Không đọc full log blob rồi truncate. Với `attempt_id`: dùng tail read (last N bytes). Với `project_id`/all: cap số attempts (≤15), tail read mỗi attempt. Cost O(limit) hoặc O(attempts × tail_bytes), không phải O(total_log_size).

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Agent logs retrieved",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "attempt_id": "...",
      "task_id": "...",
      "task_title": "Task Title",
      "project_name": "Project Name",
      "log_type": "system",
      "content": "Log message",
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/hooks/useDashboard.ts`

**Màn hình**: Dashboard Page

**Component**: `AgentFeed.tsx`

**Backend**: `crates/server/src/routes/agent_activity.rs::get_agent_logs`

---

### 3. GET `/api/v1/agent/status`

Lấy agent provider status.

Provider được lấy từ `system_settings.agent_cli_provider` (cấu hình trong Settings UI).

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Agent status retrieved successfully",
  "data": {
    "provider": "claude-code",
    "connected": true,
    "message": "Claude Code CLI is connected and ready",
    "session_info": {
      "session_dir": "/root/.claude",
      "project_count": 3
    }
  }
}
```

**Notes**:
- `provider` ∈ `claude-code` | `openai-codex` | `gemini-cli` | `cursor` (canonical; aliases `codex`/`gemini` normalized on save)
- `session_info` hiện chỉ có cho `claude-code` (đọc từ `~/.claude`)

#### Frontend Usage

**File**: `frontend/src/pages/DashboardPage.tsx`

**Màn hình**: Dashboard Page

**Backend**: `crates/server/src/routes/agent.rs::get_agent_status`

---

### 4. GET `/api/v1/agent/providers/status`

Lấy status tất cả supported providers.

Xem chi tiết tại [22-agent-provider-auth.md](./22-agent-provider-auth.md).

---

### 5. Agent Provider Auth Endpoints

Các endpoints cho agent authentication flow (`/api/v1/agent/auth/*`).

Xem chi tiết tại [22-agent-provider-auth.md](./22-agent-provider-auth.md).

---

## Agent Status Values

- `queued`: Waiting in queue
- `running`: Currently executing
- `success`: Completed successfully
- `failed`: Failed with error
- `cancelled`: Cancelled by user

## Log Types

- `system`: System messages
- `stdout`: Standard output
- `stderr`: Standard error

## Permissions

- **Get Agent Status**: Tất cả authenticated users
- **Get Agent Logs**: Tất cả authenticated users
- **Get Provider Status**: Tất cả authenticated users
