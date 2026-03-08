# OpenClaw Gateway: 05 - Webhook Engine

## 1. Concept

To avoid OpenClaw aggressively polling mirrored status endpoints such as `GET /api/openclaw/v1/attempts/{id}`, Agentic-Coding implements an async Webhook pushing capability. When significant state changes occur internally, the `Webhook Dispatcher` formats the event and pushes it to OpenClaw's registered receiver URL.

## 2. Event Dispatching Architecture

1.  **In-App Event Bus**: In `crates/executors/src/orchestrator.rs` and related execution/session state management, whenever a task or attempt state changes, an internal event is emitted.
2.  **Background Worker**: The `Webhook Dispatcher` listens to these events. It operates in an asynchronous background Tokio task to ensure it NEVER blocks the main execution flow or API responses.
3.  **HTTP Client**: `reqwest` is used to send a `POST` request to the configured external webhook URL.

## 3. Webhook Registration Configuration

Currently, for v1 simplicity, instead of dynamic registrations, the target Webhook URL can be stored in the configurations (or via a simple Settings UI later). A preferred future path is to let `POST /api/openclaw/guide-for-openclaw` accept and persist OpenClaw's `webhook_receiver_url` during bootstrap.
*   `OPENCLAW_WEBHOOK_URL=https://openclaw.system/api/agentic-events`

## 4. Security: HMAC-SHA256 Signature Validation

Webhooks are vulnerable to spoofing if left unprotected. We will use the `OPENCLAW_WEBHOOK_SECRET` generated during installation to secure payloads.

### 4.1 Dispatch Sequence (Agentic-Coding side)

1.  Construct the Event JSON payload. Example:
    ```json
    {
      "event": "attempt.completed",
      "timestamp": "2026-03-06T12:00:00Z",
      "data": {
        "attempt_id": "uuid-123",
        "task_id": "uuid-456",
        "status": "success",
        "summary": "Completed Webhook Dispatcher implementation."
      }
    }
    ```
2.  Serialize the payload to bytes.
3.  Compute the HMAC-SHA256 hash using the raw JSON bytes and the `OPENCLAW_WEBHOOK_SECRET`. Represent the output in a hex string (e.g., `a1b2c3...`).
4.  Append an HTTP Header to the outbound `POST` request:
    `X-Agentic-Signature: a1b2c3...`
5.  Send the request.

### 4.2 Verification Sequence (OpenClaw side - Informational)

OpenClaw must do the following to verify the authenticity:
1.  Read the raw HTTP request body bytes natively.
2.  Extract the `X-Agentic-Signature` header.
3.  Compute an HMAC-SHA256 hash utilizing the exact raw bytes received and its local copy of the Webhook Secret.
4.  Perform a constant-time string comparison between the computed hash and the header signature.
5.  If they match perfectly: Accept the data. If not: Drop the request (`401 Unauthorized`).

## 5. Typical Event Types

*   `task.status_changed`: Fired when a Kanban item moves columns.
*   `attempt.started`: Fired when an execution attempt begins running.
*   `attempt.completed`: Fired when the attempt exits normally. Data payload includes summary/diff metadata where available.
*   `attempt.failed`: Fired when the attempt crashes, times out, or otherwise terminates unsuccessfully.
*   `attempt.needs_input`: Fired when the running attempt pauses and requires operator input.
