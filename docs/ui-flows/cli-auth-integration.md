# Web UI to CLI Authentication Integration

Tracking checklist: [UI Agent Authentication - Ticket Breakdown Checklist](./auth-ui-ticket-breakdown-checklist.md)

## 0. Current Implementation Snapshot (2026-02-27)

This section is the source of truth for the currently implemented contract.

### 0.1 Feature Flag
- Server-side flag: `AGENT_UI_AUTH_ENABLED`
- Default behavior: enabled when env var is missing
- Disable values: `0`, `false`, `off`, `no`
- Scope: blocks auth REST endpoints and auth session WebSocket endpoint

### 0.2 REST Endpoints
- `GET /api/v1/agent/providers/status`
- `POST /api/v1/agent/auth/initiate`
- `POST /api/v1/agent/auth/submit-code`
- `POST /api/v1/agent/auth/cancel`
- `GET /api/v1/agent/auth/sessions/:id`

### 0.3 WebSocket Endpoint
- `GET /api/v1/agent/auth/sessions/:id/ws?since_seq=...`
- Message types:
  - `snapshot` with `{ sequence_id, session }`
  - `upsert` with `{ sequence_id, session }`
  - `gap_detected` with `{ requested_since_seq, max_available_sequence_id }`

### 0.4 Current Session Model Fields
- `session_id`, `provider`, `flow_type`, `status`
- `created_at`, `updated_at`, `expires_at`
- `process_pid`, `allowed_loopback_port`, `last_seq`
- `last_error`, `result`
- `action_url`, `action_code`, `action_hint`

### 0.5 Security Guards Implemented
- session ownership check on all auth endpoints and auth WS endpoint
- rate limit on submit endpoint per session window
- strict localhost callback validation (`127.0.0.1` / `localhost` only)
- callback port validation against session loopback port when available
- non-localhost HTTP/HTTPS callback URLs are rejected
- callback proxy uses `reqwest` with redirect following disabled

## 1. Overview
This document outlines the architecture and unified flow for authenticating CLI providers (`claude-code`, `openai-codex`, and `gemini-cli`) directly from the Web UI. 
Currently, the system requires users to authenticate manually via the terminal. This feature allows users to perform the authentication process entirely from the Web UI.

### 1.1 General Architecture
- **Frontend (React UI):** Displays unified auth modals, URLs, and input fields. Communicates via REST/WebSocket using a tracked `auth_session_id`.
- **Backend (Rust):** Spawns the CLI as a child process, intercepts `stdout` to parse URLs/codes, masks sensitive data in logs, and pipes `stdin` or acts as a local proxy to complete the flow.

---

## 2. API & WebSocket Contracts (Unified)

Auth operations are isolated by `auth_session_id` to prevent cross-session injection.

### 2.1 Initiate Auth (REST)
- **Request:** `POST /api/v1/agent/auth/initiate`
  ```json
  { "provider": "gemini-cli" }
  ```
- **Response:** `200 OK`
  ```json
  { "session_id": "uuid-1234-abcd", "status": "initiated" }
  ```

### 2.2 WebSocket Events (Backend to UI)
All events belonging to an auth flow include the `session_id`. Sensitive codes are masked in logs (e.g., `ABCD-****`), but sent fully to the specific WS client.

```json
{
  "event": "AUTH_REQUIRED_ACTION",
  "data": {
    "session_id": "uuid-1234-abcd",
    "provider": "gemini-cli",
    "action_type": "OOB_URL", // or "DEVICE_FLOW", "LOOPBACK_PROXY"
    "url": "https://accounts.google.com/...",
    "device_code": null // Used for Codex
  }
}
```

### 2.3 Submit Code or Redirect URL (REST)
Used when the user must provide an OOB code or a blocked localhost redirect URL.
- **Request:** `POST /api/v1/agent/auth/submit-code`
  ```json
  {
    "session_id": "uuid-1234-abcd",
    "code": "4/0AeaY..." // The auth code or the full localhost callback URL
  }
  ```
- **Response:** `200 OK`

### 2.4 Final Status Event (Backend to UI)
```json
{
  "event": "AUTH_SUCCESS", // or "AUTH_FAILED"
  "data": {
    "session_id": "uuid-1234-abcd",
    "provider": "gemini-cli"
  }
}
```

---

## 3. Security & Timeout Policies
1. **Redaction**: Raw codes, tokens, and sensitive query params MUST be redacted in stdout logging (`***MASKED***`).
2. **Session Isolation**: The `submit-code` endpoint strictly verifies that the `session_id` matches the user's active session.
3. **Timeouts & Cleanup**: Any initiated auth process is hard-killed (`child.kill()`) after **5 minutes** to prevent zombie processes.
4. **Cancellation**: If the user closes the modal or disconnects from the WebSocket, the backend aborts the session and kills the child process.
