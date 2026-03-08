# OpenClaw Gateway: 01 - Overview & Architecture

## 1. Goal & Objectives

The primary goal of the **OpenClaw Gateway** feature is to let `OpenClaw` operate Agentic-Coding as a remote **Super Admin** control plane.

This means the gateway is **not** a narrow integration with a few hand-picked endpoints. Instead, it is a secure external surface that exposes the same internal product capabilities that a trusted system administrator can access inside Agentic-Coding.

OpenClaw must be able to:

1.  **Access the Full Admin API Surface**: Read the same server-side business and administrative data available internally, including Projects, Tasks, Requirements, Sprints, Reviews, Execution state, Settings, Users, Deployments, and integration status.
2.  **Control the Entire System**: Trigger, cancel, resume, inspect, and steer any workflow that a system administrator can perform through the existing backend APIs.
3.  **Bootstrap Itself**: Call a dedicated `guide-for-openclaw` bootstrap API to receive an instance-specific instruction prompt, connection checklist, and setup flow before using the rest of the mirrored API surface.
4.  **Auto-discover Capabilities**: Fetch a complete OpenAPI description of the mirrored internal API surface so OpenClaw can dynamically generate tools instead of relying on custom adapters.
5.  **Act as an Operations Assistant**: Behave as an operations assistant for the primary human user, not just as a raw API client. It should interpret ACPMS state, guide workflows, and surface actionable updates.
6.  **Analyze User Requirements with ACPMS Context**: Combine the user's requirement with ACPMS data such as project status, existing tasks, requirements, architecture, sprint state, execution history, and integrations to propose a solution path.
7.  **Turn Analysis into Action**: Convert approved solutions into ACPMS operations such as creating requirements, creating tasks, assigning work, and starting execution attempts.
8.  **Report to Human Channels**: Deliver summaries, alerts, progress updates, and recommended actions back to the primary user via OpenClaw-managed channels such as Telegram or Slack.
9.  **Receive Real-time Updates**: Consume outbound-only SSE and WebSocket-compatible streams by default for long-running processes and state changes, with Webhooks remaining optional.
10. **Remain Auditable and Revocable**: Because the credential is effectively root-level for the product, every request must be attributable, reviewable, and easy to revoke.

## 2. Architectural Design Choices

To fulfill the requirements above, the chosen architecture pattern is **Full Internal API Mirroring + Dedicated Gateway Authentication + OpenAPI + Stream-First Event Delivery**.

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
*   **Why stream-first instead of Webhook-first?**
    *   The user may not want OpenClaw to expose a public inbound domain or receiver endpoint.
    *   A long-lived outbound connection from OpenClaw to ACPMS works in stricter network environments and keeps the OpenClaw side private.
    *   A global event stream plus attempt-specific log streams covers both lifecycle notifications and real-time execution visibility.
*   **Why keep Webhooks optional?**
    *   Some deployments may still want ACPMS -> OpenClaw push delivery for redundancy or integration with an existing inbound control plane.
    *   The same event model can be delivered over either transport without changing ACPMS domain semantics.
*   **Why HMAC-SHA256?**
    *   To verify the authenticity of outbound Webhook payloads in deployments that enable optional Webhook delivery.
    *   It ensures that the payload originated from Agentic-Coding and has not been tampered with in transit.

## 3. High-Level Flow

1.  **Provisioning**: User installs Agentic-Coding and opts to enable the OpenClaw Gateway. The installer generates an API Key and may also generate a Webhook Secret for optional Webhook mode.
2.  **Configuration**: User stores those credentials inside OpenClaw as a privileged integration.
3.  **Bootstrap Guide Call**: OpenClaw first calls `POST /api/openclaw/guide-for-openclaw` using the API Key. The response returns an instruction prompt, required headers, stream URLs, optional Webhook rules, endpoint checklist, and connection status.
4.  **Discovery**: OpenClaw fetches `/api/openclaw/openapi.json` to discover the mirrored internal API surface.
5.  **Primary Event Subscription**: OpenClaw opens the global ACPMS event stream and keeps it connected so it can receive attempt lifecycle events, task status changes, approval requests, and system alerts without exposing an inbound endpoint.
6.  **Optional Webhook Finalization**: If a deployment intentionally enables Webhooks and OpenClaw includes receiver metadata (for example `webhook_receiver_url`) in the bootstrap call, ACPMS stores that information so outbound Webhooks can be completed.
7.  **Gateway Authentication**: OpenClaw calls `/api/openclaw/v1/...` using `Authorization: Bearer <OPENCLAW_API_KEY>`.
8.  **Identity Translation**: The gateway validates the token and injects a synthetic `OpenClaw Super Admin` identity into request handling.
9.  **Normal Backend Execution**: Existing Rust handlers and services process the request using the same domain logic as the internal product APIs.
10. **Streaming and Optional Webhooks**: Long-running attempts emit live log streams, lifecycle events flow through the global event stream, and optional signed Webhooks may be emitted if configured.
11. **Audit Trail**: Every OpenClaw request is recorded with request metadata so administrators can trace what the external automation layer did.
