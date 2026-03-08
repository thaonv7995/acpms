# OpenClaw Gateway: 07 - Streaming APIs (SSE)

## 1. Overview

The default OpenClaw transport model should be **outbound-only**: OpenClaw connects to ACPMS over authenticated SSE streams and does not need to expose a public inbound receiver.

The **Streaming APIs** use Server-Sent Events (SSE) for two distinct purposes:

1.  a **global event stream** for lifecycle and business-state changes
2.  an **attempt-specific stream** for real-time logs, stdout, and stderr from an actively running execution attempt

SSE is highly resilient, works over standard HTTP/1.1 or HTTP/2, and is natively supported by most modern HTTP clients without the overhead of two-way WebSockets.

## 2. API Specification

### 2.1 Global Event Stream

*   **Endpoint**: `GET /api/openclaw/v1/events/stream`
*   **Headers Required**:
    *   `Authorization: Bearer <OPENCLAW_API_KEY>`
    *   `Accept: text/event-stream`
*   **Resume Support**:
    *   `Last-Event-ID: <event_id>` header, or
    *   `GET /api/openclaw/v1/events/stream?after=<event_id>`

#### 2.1.1 Connection Behavior
1.  **Authentication**: The server validates the Bearer token.
2.  **Replay Cursor**: If `Last-Event-ID` or `after` is provided, the server replays missed events from the event journal when available.
3.  **Stream Initialization**: The server responds with `200 OK` and `Content-Type: text/event-stream`.
4.  **Event Delivery**: The server subscribes to the internal event bus and forwards lifecycle/business events encoded as JSON inside SSE data blocks.
5.  **Long-lived Session**: The stream is expected to remain connected for the duration of the OpenClaw session and reconnect automatically after disconnects.

#### 2.1.2 Typical Event Types

```text
event: attempt.started
id: evt_000001
data: {"attempt_id":"uuid-123","task_id":"uuid-456","status":"running"}
```

```text
event: attempt.completed
id: evt_000002
data: {"attempt_id":"uuid-123","task_id":"uuid-456","status":"success","summary":"Completed implementation"}
```

```text
event: task.status_changed
id: evt_000003
data: {"task_id":"uuid-456","old_status":"in_progress","new_status":"in_review"}
```

```text
event: attempt.needs_input
id: evt_000004
data: {"attempt_id":"uuid-123","prompt_text":"Please confirm production deploy","timeout_seconds":300}
```

The global event stream is the primary mechanism OpenClaw should use to know when an attempt has started, completed, failed, or is waiting for input.

### 2.2 Attempt Log Stream

*   **Endpoint**: `GET /api/openclaw/v1/attempts/{attempt_id}/stream`
*   **Headers Required**:
    *   `Authorization: Bearer <OPENCLAW_API_KEY>`
    *   `Accept: text/event-stream`

#### 2.2.1 Connection Behavior
1.  **Authentication**: The server validates the Bearer token.
2.  **Attempt Lookup**: The server checks if the requested `{attempt_id}` exists and is accessible. If it does not exist, it returns `404 Not Found`.
3.  **Stream Initialization**: The server responds with `200 OK` and `Content-Type: text/event-stream`.
4.  **Event Delivery**: The server subscribes to the internal `broadcast_tx` event bus (from `crates/executors/src/orchestrator.rs`) and forwards `AgentEvent` payloads encoded as JSON inside SSE data blocks.
5.  **Termination**: The stream will automatically close when the attempt transitions to a terminal state (`Success`, `Failed`, or `Cancelled`), or if the client disconnects.

## 3. SSE Event Format

Events are sent as standard Server-Sent Event text streams. Each event has an `event` type and a `data` JSON payload. The global event stream should also include stable `id:` values for resume/replay support.

### Standard Log Event
```text
event: log
data: {"timestamp": "2026-03-06T12:00:00Z", "source": "stdout", "content": "Running npm install..."}
```

### State Change Event
```text
event: state_change
data: {"status": "running"}
```

### Stream Termination
```text
event: end
data: {"reason": "completed", "exit_code": 0}
```

## 4. Backend Implementation Requirements (Rust / Axum)

In the `crates/server` backend:
*   We will leverage `axum::response::sse::{Event, Sse, KeepAlive}` which is already used in the system (`routes/streams.rs`).
*   The attempt log handler converts the `tokio::sync::broadcast::Receiver<AgentEvent>` into an async `Stream` (`tokio_stream::wrappers::BroadcastStream`).
*   The global event stream handler should read from a durable event journal or replay buffer so reconnecting OpenClaw clients can recover missed lifecycle events using `Last-Event-ID` or `after=<cursor>`.
*   A heartbeat (`KeepAlive`) must be configured to send empty SSE comments (e.g., `: keep-alive`) every 10-15 seconds to prevent Load Balancers, Cloudflare tunnels, or Reverse Proxies from aggressively closing the supposedly "idle" HTTP connection when the Agent is silently thinking for a long time.

The attempt log endpoint should mirror the existing internal attempt stream rather than introducing a separate OpenClaw-only session abstraction. The global event stream is gateway-specific and exists so OpenClaw can operate without inbound Webhooks.
