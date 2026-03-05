# OpenClaw Gateway: 07 - Streaming API (SSE)

## 1. Overview

While Webhooks provide asynchronous updates for major state changes (e.g., Session Complete, Task Failed), some external LLM clients or UIs require real-time visibility into the Agent's thought process and execution logs. 

The **Streaming API** uses Server-Sent Events (SSE) to deliver real-time logs, stdout, and stderr from the actively running executor session to the connected OpenClaw instance. SSE is highly resilient, works over standard HTTP/1.1 or HTTP/2, and is natively supported by most modern HTTP clients without the overhead of two-way WebSockets.

## 2. API Specification

*   **Endpoint**: `GET /api/openclaw/v1/sessions/{session_id}/stream`
*   **Headers Required**:
    *   `Authorization: Bearer <OPENCLAW_API_KEY>`
    *   `Accept: text/event-stream`

### 2.1 Connection Behavior
1.  **Authentication**: The server validates the Bearer token.
2.  **Session Lookup**: The server checks if the requested `{session_id}` exists and is accessible. If it does not exist, it returns `404 Not Found`.
3.  **Stream Initialization**: The server responds with `200 OK` and `Content-Type: text/event-stream`.
4.  **Event Delivery**: The server subscribes to the internal `broadcast_tx` event bus (from `crates/executors/src/orchestrator.rs`) and forwards `AgentEvent` payloads encoded as JSON inside SSE data blocks.
5.  **Termination**: The stream will automatically close when the session transitions to a terminal state (`Completed`, `Failed`, or `Paused`), or if the client disconnects.

## 3. SSE Event Format

Events are sent as standard Server-Sent Event text streams. Each event has an `event` type and a `data` JSON payload.

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
*   We will leverage `axum::response::sse::{Event, Sse, KeepAlive}` which is already heavily used in the system (`routes/streams.rs`).
*   The handler converts the `tokio::sync::broadcast::Receiver<AgentEvent>` into an async `Stream` (`tokio_stream::wrappers::BroadcastStream`).
*   A heartbeat (`KeepAlive`) must be configured to send empty SSE comments (e.g., `: keep-alive`) every 10-15 seconds to prevent Load Balancers, Cloudflare tunnels, or Reverse Proxies from aggressively closing the supposedly "idle" HTTP connection when the Agent is silently thinking for a long time.
