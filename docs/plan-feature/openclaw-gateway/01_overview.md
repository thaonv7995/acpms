# OpenClaw Gateway: 01 - Overview & Architecture

## 1. Goal & Objectives

The primary goal of the **OpenClaw Gateway** feature is to establish a secure, two-way, and standardized communication channel. This allows the `OpenClaw` system (or any other external AI Agent platforms) to:

1.  **Access (Read)**: Retrieve structured data from the Agentic-Coding system, including Projects, Tasks, System Configuration, and Orchestrator state.
2.  **Control (Write/Execute)**: Grant permissions to issue commands to the system (e.g., create a new task, trigger an Agent session, stop an ongoing session).
3.  **Auto-discovery (Swagger/OpenAPI)**: Provide OpenAPI documentation so that OpenClaw can automatically understand and convert the endpoints into callable "Tools" without rigid hardcoding.
4.  **Real-time Reporting (Webhooks)**: Implement a Webhook engine to proactively push updates to OpenClaw when a task state changes, eliminating the need for constant polling.

## 2. Architectural Design Choices

To fulfill the requirements above, the chosen architecture pattern is **Public REST API + Webhooks with HMAC-SHA256 Signature + OpenAPI (Swagger)**.

*   **Why REST API?**
    *   REST APIs are the industry standard for system integrations.
    *   They are easily documented using OpenAPI, which is natively understood by modern LLMs and agent frameworks like OpenClaw to dynamically generate Tool schemas.
*   **Why Webhooks instead of WebSocket/Polling?**
    *   Polling is inefficient and can overload the server, especially when agent sessions run for a long time.
    *   Webhooks provide an event-driven mechanism: Agentic-Coding pushes data only when an actionable event occurs.
    *   Webhooks are stateless and robust; if a delivery fails, it can be queued and retried.
*   **Why HMAC-SHA256?**
    *   To verify the authenticity of the Webhook payload. It ensures that the payload originated from Agentic-Coding and has not been tampered with or intercepted (Man-in-the-Middle) by malicious actors.

## 3. High-Level Flow

1.  **Provisioning**: User installs Agentic-Coding via `install.sh` and opts to enable the OpenClaw Gateway. The script generates an API Key and Webhook Secret.
2.  **Configuration**: User copies the generated credentials and configures OpenClaw.
3.  **Discovery**: OpenClaw fetches `/api/openclaw/openapi.json` to understand the available endpoints and their required payloads.
4.  **Execution Request**: OpenClaw calls a control API (e.g., `POST /api/openclaw/v1/orchestrator/trigger`) using the API Key in the `Authorization` header.
5.  **Task Processing**: The Rust backend (`crates/server` -> `crates/executors`) validates the request and begins processing the task (session starts).
6.  **Event Broadcasting**: As the task progresses or finishes, the orchestrator triggers an event.
7.  **Webhook Delivery**: The Webhook Dispatcher constructs a JSON payload, signs it using HMAC-SHA256 and the Webhook Secret, and issues an HTTP POST request to OpenClaw's registered receiver URI. OpenClaw validates the signature and updates its internal state.
