# OpenClaw Gateway: 04 - API Specifications

This document defines the **full mirrored internal API contract** for the `/api/openclaw` namespace.

OpenClaw is intended to operate Agentic-Coding as a **Super Admin**. Therefore the gateway should not be limited to a small custom integration surface. Instead, it should expose the same internal business/admin APIs that the backend already provides, while using OpenClaw-specific authentication and auditing.

All endpoints (except `openapi.json`) require:
`Authorization: Bearer <OPENCLAW_API_KEY>`

---

## 1. Mirroring Rule

### 1.1 REST + SSE

For every internal REST/SSE endpoint that belongs to the server-side admin/business API surface:

*   **Internal Path**: `/api/v1/<path>`
*   **OpenClaw Path**: `/api/openclaw/v1/<path>`

The mirrored endpoint must preserve:

*   HTTP method
*   path parameters
*   query parameters
*   request body schema
*   response body schema
*   status-code semantics
*   side effects and domain behavior

### 1.2 WebSocket

For every relevant root WebSocket endpoint:

*   **Internal Path**: `/ws/<path>`
*   **OpenClaw Path**: `/api/openclaw/ws/<path>`

### 1.3 OpenAPI Export

The mirrored routes must be represented in:

*   `GET /api/openclaw/openapi.json`
*   `GET /api/openclaw/swagger-ui`

### 1.4 Bootstrap Guide Endpoint

The gateway also exposes one OpenClaw-specific bootstrap endpoint outside the mirrored `/api/openclaw/v1/*` surface:

*   **Endpoint**: `POST /api/openclaw/guide-for-openclaw`
*   **Purpose**: Give OpenClaw an instance-specific setup guide immediately after the ACPMS credentials are pasted into OpenClaw.
*   **Auth**: Requires `Authorization: Bearer <OPENCLAW_API_KEY>`
*   **Behavior**:
    *   validates the gateway API key
    *   returns an `instruction_prompt` telling OpenClaw what ACPMS is, what role OpenClaw has, and how it must operate
    *   returns the ACPMS endpoint map (`base`, `openapi`, global event stream URL, attempt stream URLs, optional webhook verification header names, etc.)
    *   serves as the authoritative runtime follow-up after the installer-generated OpenClaw prompt handoff
    *   optionally accepts OpenClaw connection metadata and persists it

Example request:

```json
{
  "openclaw_instance": {
    "name": "OpenClaw Production",
    "version": "1.0.0",
    "base_url": null
  },
  "connection": {
    "delivery_mode": "streaming",
    "webhook_receiver_url": null,
    "supports_webhooks": false,
    "supports_sse": true,
    "supports_websocket": true
  },
  "reporting": {
    "primary_user": {
      "display_name": "Alice",
      "timezone": "Asia/Ho_Chi_Minh",
      "preferred_language": "vi"
    },
    "channels": [
      {
        "type": "telegram",
        "target": "@alice_ops"
      },
      {
        "type": "slack",
        "target": "#acpms-alerts"
      }
    ]
  }
}
```

Example response shape:

```json
{
  "success": true,
  "code": "0000",
  "message": "OpenClaw bootstrap guide generated successfully",
  "data": {
    "instruction_prompt": "You are OpenClaw connected to ACPMS as a Super Admin integration...",
    "core_missions": [
      "Bootstrap ACPMS correctly by calling the guide first, loading the OpenAPI contract, storing the Bearer credential, and maintaining the global event stream connection.",
      "Build and maintain situational awareness by reading ACPMS projects, requirements, sprints, tasks, attempts, execution processes, approvals, repository context, and recent events before proposing changes.",
      "Translate human goals into explicit ACPMS execution plans with scope, dependencies, risks, acceptance criteria, and the smallest safe next actions.",
      "Create or update ACPMS artifacts when appropriate, including requirements, tasks, attempts, and supporting metadata, so ACPMS stays aligned with the real execution plan.",
      "Start, observe, and steer execution through ACPMS by tracking attempt state, blockers, approvals, failures, deployment risk, and completion signals.",
      "Report to the primary human user in a structured way: what ACPMS says, what you concluded, what you changed, what is blocked, and what decision or next step is needed.",
      "Protect ACPMS integrity by using only OpenClaw routes, treating ACPMS as the source of truth, and confirming before destructive or high-impact actions unless autonomous mode was explicitly enabled."
    ],
    "acpms_endpoints": {
      "base_endpoint_url": "https://api.example.com/api/openclaw/v1",
      "openapi_url": "https://api.example.com/api/openclaw/openapi.json",
      "guide_url": "https://api.example.com/api/openclaw/guide-for-openclaw",
      "events_stream_url": "https://api.example.com/api/openclaw/v1/events/stream",
      "websocket_base_url": "wss://api.example.com/api/openclaw/ws"
    },
    "operating_model": {
      "role": "operations_assistant",
      "human_reporting_required": true,
      "preferred_reporting_channels": ["telegram", "slack"]
    },
    "operating_rules": {
      "rulebook_version": "v1",
      "default_autonomy_mode": "analyze_then_confirm",
      "must_load_acpms_context_before_mutation": true,
      "must_report_material_changes": true,
      "must_confirm_before_destructive_actions": true,
      "high_priority_report_events": [
        "attempt_failed",
        "attempt_needs_input",
        "approval_required",
        "deployment_risk",
        "system_health_issue"
      ],
      "recommended_reporting_template": [
        "what the user asked",
        "what ACPMS context was checked",
        "what was concluded",
        "what ACPMS action was taken, if any",
        "current status",
        "next step or approval needed"
      ]
    },
    "connection_status": {
      "primary_transport": "sse_events_stream",
      "webhook_registered": false,
      "missing_steps": []
    },
    "setup_steps": [
      "Load the OpenAPI contract",
      "Open the global ACPMS event stream and keep it connected",
      "Configure Telegram or Slack reporting for the primary user",
      "Use ACPMS context when analyzing user requirements",
      "Use the mirrored /api/openclaw/v1 routes for all ACPMS operations",
      "Store the webhook secret only if optional ACPMS webhooks are enabled"
    ]
  }
}
```

---

## 2. Exposed Capability Groups

The gateway is expected to expose the same broad capability groups that exist in the internal backend.

### 2.1 Platform & Administration

OpenClaw should be able to access operational and administrative APIs such as:

*   dashboard summaries
*   health/readiness/liveness endpoints when intentionally mirrored
*   system/application settings
*   templates, previews, and other admin-managed resources

### 2.2 Identity, Users, and Membership

OpenClaw should be able to manage the same user and project-membership resources available internally, including:

*   user listing and inspection
*   user updates and deletion
*   avatar/upload helper APIs
*   project membership and role management

### 2.3 Projects and Repository Management

OpenClaw should be able to use the full project-management surface, including:

*   project CRUD
*   import and preflight flows
*   repository context inspection and re-check
*   fork linking/creation
*   architecture metadata
*   project settings and sync operations

### 2.4 Requirements, Planning, Tasks, and Sprints

OpenClaw should be able to use the complete work-management surface, including:

*   requirements CRUD
*   requirement breakdown sessions and confirmations
*   task CRUD and task metadata updates
*   kanban/task listing and task relationships
*   sprint CRUD, generation, activation, closing, and overview

### 2.5 Agent Execution and Session Control

OpenClaw should be able to fully operate agent execution, including:

*   create task attempts
*   inspect attempts and attempt skills
*   stream logs and status
*   send input to running attempts
*   cancel/stop attempts
*   inspect execution processes
*   retrieve raw logs, normalized logs, diffs, summaries, and subagent trees
*   trigger follow-up and reset flows where supported

### 2.6 Eventing and Real-time Observability

OpenClaw should be able to keep itself synchronized with ACPMS using:

*   a global outbound-only event stream for lifecycle updates
*   attempt-specific SSE streams for logs and fine-grained execution visibility
*   optional Webhook delivery when the deployment intentionally configures an inbound receiver
*   replay/resume semantics so OpenClaw can recover after connection interruptions

### 2.7 Collaboration, Review, and Approvals

OpenClaw should be able to access and operate:

*   reviews and comments
*   approval queues and responses
*   human-in-the-loop routing APIs

### 2.8 Deployments and External Integrations

OpenClaw should be able to use deployment/integration APIs such as:

*   build and deploy triggers
*   deployment runs and artifacts
*   GitLab status, merge requests, and project linking
*   webhook administration and retry tooling

### 2.9 Project Assistant APIs

OpenClaw should be able to drive project assistant sessions as part of the mirrored admin surface, including:

*   session creation/listing
*   message posting
*   status polling
*   start/end/input/confirmation flows

---

## 3. Representative Path Mapping Examples

The following examples illustrate the mirroring rule:

| Internal Endpoint | OpenClaw Endpoint | Purpose |
| :--- | :--- | :--- |
| `GET /api/v1/projects` | `GET /api/openclaw/v1/projects` | List projects |
| `PUT /api/v1/projects/{id}` | `PUT /api/openclaw/v1/projects/{id}` | Update project |
| `GET /api/v1/tasks?project_id=...` | `GET /api/openclaw/v1/tasks?project_id=...` | Kanban/task listing |
| `POST /api/v1/tasks/{task_id}/attempts` | `POST /api/openclaw/v1/tasks/{task_id}/attempts` | Start agent execution |
| `(gateway-specific)` | `GET /api/openclaw/v1/events/stream` | Global lifecycle event stream |
| `GET /api/v1/attempts/{id}` | `GET /api/openclaw/v1/attempts/{id}` | Inspect attempt |
| `GET /api/v1/attempts/{id}/stream` | `GET /api/openclaw/v1/attempts/{id}/stream` | SSE streaming |
| `POST /api/v1/attempts/{id}/input` | `POST /api/openclaw/v1/attempts/{id}/input` | Human input / steering |
| `POST /api/v1/attempts/{id}/cancel` | `POST /api/openclaw/v1/attempts/{id}/cancel` | Emergency stop |
| `GET /ws/attempts/{id}/logs` | `GET /api/openclaw/ws/attempts/{id}/logs` | Live log WebSocket |
| `GET /ws/projects/{project_id}/agents` | `GET /api/openclaw/ws/projects/{project_id}/agents` | Project agent presence |

---

## 4. Exceptions

Even under the “full internal API” goal, a few categories are intentionally outside the main mirrored contract:

1.  **User Auth Bootstrap**: Login/register/refresh/logout flows are not needed because OpenClaw uses its dedicated bearer token.
2.  **Gateway Bootstrap Endpoint**: `POST /api/openclaw/guide-for-openclaw` is intentionally custom to help OpenClaw self-configure.
3.  **Gateway Event Stream**: `GET /api/openclaw/v1/events/stream` is intentionally custom so OpenClaw can receive outbound-only lifecycle events without exposing an inbound receiver.
4.  **Browser Redirect Callbacks**: Human-interactive OAuth callback endpoints remain browser-oriented.
5.  **Raw Scrape/Debug Endpoints**: Non-OpenAPI operational endpoints such as raw Prometheus scrapes can be exposed separately if desired, but they are not part of the main mirrored OpenClaw contract by default.

---

## 5. Compatibility Contract

The gateway must prioritize parity over simplification:

*   Do **not** invent a second OpenClaw-only DTO model for resources that already exist internally.
*   Do **not** flatten or rename fields just to make them look cleaner for LLMs.
*   Do **not** silently drop admin capabilities from the mirrored spec.
*   New internal admin/business APIs should be added to the OpenClaw mirror as part of the same rollout whenever possible.
