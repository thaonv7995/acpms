# OpenClaw Gateway: 01 - Overview & Architecture

## 1. Goal & Objectives

The primary goal of the **OpenClaw Gateway** feature is to let `OpenClaw` operate Agentic-Coding as a remote **Super Admin** control plane.

This means the gateway is **not** a narrow integration with a few hand-picked endpoints. Instead, it is a secure external surface that exposes the same internal product capabilities that a trusted system administrator can access inside Agentic-Coding.

OpenClaw must be able to:

1.  **Access the Full Admin API Surface**: Read the same server-side business and administrative data available internally, including Projects, Tasks, Requirements, Sprints, Reviews, Execution state, Settings, Users, Deployments, and integration status.
2.  **Control the Entire System**: Trigger, cancel, resume, inspect, and steer any workflow that a system administrator can perform through the existing backend APIs.
3.  **Auto-discover Capabilities**: Fetch a complete OpenAPI description of the mirrored internal API surface so OpenClaw can dynamically generate tools instead of relying on custom adapters.
4.  **Receive Real-time Updates**: Consume Webhooks, SSE, and WebSocket-compatible streams for long-running processes and state changes.
5.  **Remain Auditable and Revocable**: Because the credential is effectively root-level for the product, every request must be attributable, reviewable, and easy to revoke.

## 2. Architectural Design Choices

To fulfill the requirements above, the chosen architecture pattern is **Full Internal API Mirroring + Dedicated Gateway Authentication + OpenAPI + Webhooks/Streams**.

*   **Why mirror the internal API instead of hand-curating a subset?**
    *   Agentic-Coding already has a large and evolving backend surface. Re-modeling only a subset would create immediate drift and leave OpenClaw unable to use new product capabilities.
    *   Mirroring existing handlers, DTOs, status codes, and schemas keeps OpenClaw aligned with the real product contract.
    *   OpenClaw can act as a true automation layer for the whole system instead of only task orchestration.
*   **Why keep a dedicated OpenClaw namespace?**
    *   The external credential must be isolated from user JWT/session auth.
    *   The namespace allows separate auditing, rate limiting, revocation, monitoring, and rollout controls without changing the normal frontend API.
*   **Why model OpenClaw as a synthetic Super Admin principal?**
    *   OpenClaw is intended to operate the entire system, not one project-scoped role at a time.
    *   Mapping the gateway token to a trusted system-admin-equivalent actor avoids fragile per-endpoint permission remapping.
*   **Why Webhooks and streaming together?**
    *   Webhooks are efficient for major asynchronous state transitions.
    *   SSE/WebSocket-style streaming remains necessary for long-running agent sessions, execution logs, approvals, and live operator steering.
*   **Why HMAC-SHA256?**
    *   To verify the authenticity of outbound Webhook payloads. It ensures that the payload originated from Agentic-Coding and has not been tampered with in transit.

## 3. High-Level Flow

1.  **Provisioning**: User installs Agentic-Coding and opts to enable the OpenClaw Gateway. The installer generates an API Key and Webhook Secret.
2.  **Configuration**: User stores those credentials inside OpenClaw as a privileged integration.
3.  **Discovery**: OpenClaw fetches `/api/openclaw/openapi.json` to discover the mirrored internal API surface.
4.  **Gateway Authentication**: OpenClaw calls `/api/openclaw/v1/...` using `Authorization: Bearer <OPENCLAW_API_KEY>`.
5.  **Identity Translation**: The gateway validates the token and injects a synthetic `OpenClaw Super Admin` identity into request handling.
6.  **Normal Backend Execution**: Existing Rust handlers and services process the request using the same domain logic as the internal product APIs.
7.  **Streaming and Webhooks**: Long-running attempts emit live streams and major lifecycle changes trigger signed outbound Webhooks.
8.  **Audit Trail**: Every OpenClaw request is recorded with request metadata so administrators can trace what the external automation layer did.
