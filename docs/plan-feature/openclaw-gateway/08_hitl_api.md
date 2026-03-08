# OpenClaw Gateway: 08 - Human-in-the-Loop (HITL) API

## 1. Overview

While AI Agents are powerful, they often encounter situations where they lack context, permissions, or certainty to proceed safely. The **Human-in-the-Loop (HITL)** feature allows the Orchestrator to proactively pause an Agent's execution and request human intervention.

This is critical for:
*   Clarifying ambiguous requirements mid-task.
*   Requesting explicit permission for destructive actions (if approval mode is strict).
*   Requesting authentication codes (e.g., 2FA prompts from CLI tools).

## 2. HITL Flow Architecture

1.  **Detection**: The Executor monitors the Agent's stdout/stderr. If it detects a predefined "needs input" pattern (or if the Agent explicitly uses a `ask_human` tool), the Orchestrator pauses the execution stream.
2.  **Notification (Webhook)**: The Orchestrator fires an `attempt.needs_input` Webhook to OpenClaw.
    ```json
    {
      "event": "attempt.needs_input",
      "data": {
        "attempt_id": "uuid-123",
        "prompt_text": "I need the 2FA code to deploy to production. Please provide it:",
        "timeout_seconds": 300
      }
    }
    ```
3.  **Waiting State**: The Orchestrator puts the execution attempt into a `WaitingForInput` state. The underlying process (e.g., `claude-code`) is kept alive but suspended.
4.  **Resolution (API Call)**: OpenClaw (via the Human) calls the mirrored `POST /api/openclaw/v1/attempts/{attempt_id}/input` endpoint with the human's response.
5.  **Resumption**: The Orchestrator receives the API call, pipes the provided text into the Agent process's `stdin`, and transitions the attempt back to `Running`.

## 3. API Specification

*   **Endpoint**: `POST /api/openclaw/v1/attempts/{attempt_id}/input`
*   **Headers Required**:
    *   `Authorization: Bearer <OPENCLAW_API_KEY>`
    *   `Content-Type: application/json`

### 3.1 Payload
```json
{
  "input_text": "123456",
  "action": "submit" // "submit" or "cancel_session"
}
```

### 3.2 Expected Responses
*   `200 OK`: Input successfully routed to the running process.
*   `400 Bad Request`: Attempt is not currently in a state waiting for input.
*   `404 Not Found`: Attempt ID invalid or expired.
*   `409 Conflict`: The attempt has already timed out or was terminated.

## 4. Backend Implementation Requirements

In the `crates/executors/src/orchestrator.rs` and `crates/server` backend:
*   Extend the `ActiveSession` struct to hold a standard input channel: `input_tx: mpsc::Sender<String>`.
*   When spawning the CLI provider (Claude/Gemini/Cursor), capture `Stdio::piped()` for standard input.
*   Create a background loop that reads from `input_tx` and writes to the child process `stdin`.
*   Implement the mirrored Axum route handler so it looks up the active input channel for the `attempt_id` and sends the payload.
