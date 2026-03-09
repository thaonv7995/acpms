# OpenClaw Gateway: 09 - Bootstrap Guide API

## 1. Purpose

After the installer provides the ACPMS connection bundle:

*   `Base Endpoint URL`
*   `OpenAPI (Swagger)`
*   `API Key (Bearer)`
*   `Global Event SSE`
*   `Webhook Secret` (optional)
*   `Ready-to-Send Installer Prompt`

OpenClaw should not immediately start calling arbitrary ACPMS endpoints.

Instead, OpenClaw should first call a dedicated bootstrap endpoint:

*   `POST /api/openclaw/guide-for-openclaw`

This endpoint returns the ACPMS-specific onboarding guide for OpenClaw. It acts as the machine-readable and prompt-ready "operator manual" for this ACPMS instance.

The installer-generated prompt and the bootstrap API have different jobs:

*   the installer prompt is the **human handoff artifact**
*   `POST /api/openclaw/guide-for-openclaw` is the **authoritative runtime guide**

## 2. Why This Endpoint Exists

The OpenAPI contract explains what endpoints exist, but it does **not** fully explain:

*   OpenClaw's mission and authority inside ACPMS
*   which ACPMS capabilities OpenClaw should prefer first
*   how OpenClaw should connect to ACPMS using outbound-only event streams
*   how OpenClaw should verify Webhooks if optional Webhook mode is enabled
*   how OpenClaw should finalize the ACPMS -> OpenClaw connection without exposing an inbound domain
*   how OpenClaw should decide whether to read, analyze, mutate, execute, or only report when a user gives it a command
*   how OpenClaw should report status and incidents back to the primary human user
*   what safety and approval boundaries should apply to destructive admin actions

The `guide-for-openclaw` endpoint fills that gap by returning a structured guide plus a large instruction prompt that OpenClaw can load into its integration/runtime context.

This means the correct first-run flow is:

1.  user copies the installer-generated prompt and sends it to OpenClaw
2.  OpenClaw extracts the embedded ACPMS connection bundle
3.  OpenClaw calls `POST /api/openclaw/guide-for-openclaw`
4.  OpenClaw treats the response as the authoritative runtime contract

At a minimum, the guide should teach OpenClaw these three core missions:

1.  load ACPMS information and report meaningful status back to the primary user
2.  analyze a user's requirement using ACPMS context to propose a solution
3.  turn the approved solution into ACPMS actions such as creating tasks and running attempts

## 3. Endpoint Contract

### 3.1 Request

*   **Method**: `POST`
*   **Path**: `/api/openclaw/guide-for-openclaw`
*   **Headers**:
    *   `Authorization: Bearer <OPENCLAW_API_KEY>`
    *   `Content-Type: application/json`

### 3.2 Optional Request Payload

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

## 4. Response Shape

The response should reuse the normal ACPMS `ApiResponse<T>` envelope.

```json
{
  "success": true,
  "code": "0000",
  "message": "OpenClaw bootstrap guide generated successfully",
  "data": {
    "instruction_prompt": "You are OpenClaw connected to ACPMS as a Super Admin integration...",
    "core_missions": [
      "Load ACPMS information and report it to the primary human user",
      "Analyze user requirements using ACPMS context",
      "Propose solutions and execution plans",
      "Create tasks or requirements in ACPMS when appropriate",
      "Run and monitor task attempts after confirmation or according to autonomy policy"
    ],
    "acpms_profile": {
      "product_name": "ACPMS",
      "role": "super_admin_integration",
      "base_endpoint_url": "https://api.example.com/api/openclaw/v1",
      "openapi_url": "https://api.example.com/api/openclaw/openapi.json",
      "guide_url": "https://api.example.com/api/openclaw/guide-for-openclaw",
      "events_stream_url": "https://api.example.com/api/openclaw/v1/events/stream",
      "websocket_base_url": "wss://api.example.com/api/openclaw/ws"
    },
    "operating_model": {
      "role": "operations_assistant",
      "primary_human_relationship": "reporting_assistant",
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
    "auth_rules": {
      "rest_auth_header": "Authorization: Bearer <OPENCLAW_API_KEY>",
      "event_stream_resume": "Reconnect with Last-Event-ID or ?after=<event_id> when supported",
      "webhook_signature_header": "X-Agentic-Signature",
      "webhook_secret_usage": "Use OPENCLAW_WEBHOOK_SECRET to verify HMAC-SHA256 signatures from ACPMS when optional Webhooks are enabled"
    },
    "reporting_policy": {
      "report_to_primary_user": true,
      "notify_on": [
        "attempt_started",
        "attempt_completed",
        "attempt_failed",
        "approval_needed",
        "deployment_risk",
        "system_health_issue"
      ],
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
    },
    "connection_status": {
      "primary_transport": "sse_events_stream",
      "webhook_registered": false,
      "missing_steps": []
    },
    "setup_steps": [
      "Load the OpenAPI contract",
      "Open the global ACPMS event stream and keep it connected",
      "Configure reporting to the primary user via Telegram or Slack",
      "Use ACPMS context when analyzing user requirements",
      "Use the mirrored /api/openclaw/v1 routes for all ACPMS operations",
      "Store the webhook secret only if optional ACPMS webhooks are enabled"
    ],
    "next_calls": [
      {
        "method": "GET",
        "path": "/api/openclaw/openapi.json",
        "purpose": "Load ACPMS tool surface"
      },
      {
        "method": "GET",
        "path": "/api/openclaw/v1/events/stream",
        "purpose": "Subscribe to ACPMS lifecycle events"
      },
      {
        "method": "GET",
        "path": "/api/openclaw/v1/projects",
        "purpose": "Validate project access and enumerate workspaces"
      }
    ]
  }
}
```

## 5. The `instruction_prompt` Content

The `instruction_prompt` should be a long-form prompt intended to be copied into OpenClaw's ACPMS integration context. It should explicitly tell OpenClaw:

1.  it is connected to ACPMS as a **Super Admin integration**
2.  it must use the mirrored `/api/openclaw/v1/*` and `/api/openclaw/ws/*` surfaces
3.  it must use `/api/openclaw/openapi.json` for discovery rather than hardcoding tools
4.  it must connect to `GET /api/openclaw/v1/events/stream` as the default lifecycle transport
5.  it must verify ACPMS Webhooks using `OPENCLAW_WEBHOOK_SECRET` only if optional Webhook mode is enabled
6.  it may manage projects, tasks, sprints, requirements, attempts, settings, reviews, deployments, and integrations through ACPMS
7.  it should act like an **operations assistant** for the primary human user
8.  it should report important ACPMS events, summaries, and incidents back to that user through OpenClaw-managed channels such as Telegram or Slack
9.  it should analyze user requirements by combining the request with ACPMS context before proposing a solution
10. it should convert approved solutions into ACPMS actions such as creating requirements, creating tasks, and starting task attempts
11. it should require additional human/operator confirmation before destructive system-wide actions unless the operator has explicitly enabled fully autonomous mode
12. it should follow the ACPMS operating rules for deciding when to only report, when to propose, and when to mutate ACPMS

### 5.1 Full Example Prompt

The returned `instruction_prompt` can be as explicit as the following:

```text
You are OpenClaw connected to ACPMS (Agentic Coding Project Management System) as a trusted Super Admin integration.

Your role:
- Operate ACPMS through its OpenClaw gateway as an automation and control plane.
- Behave as an operations assistant for the primary human user, not just as a raw API client.
- Read ACPMS state, execute ACPMS actions, monitor long-running work, and report meaningful updates back to the primary user.
- Use Telegram, Slack, or other configured OpenClaw reporting channels to keep the primary user informed.
- Analyze user requirements by combining them with ACPMS context such as existing projects, requirements, tasks, sprint state, execution history, and architecture metadata.
- When appropriate, turn that analysis into ACPMS actions such as creating requirements, creating tasks, and starting task attempts.

Authority:
- You have Super Admin-equivalent access to ACPMS through the OpenClaw gateway.
- Use that authority carefully, with preference for safe, auditable, and reversible actions.
- Require explicit human confirmation before destructive or high-impact operations unless autonomous mode is explicitly enabled.

ACPMS connection rules:
- Base API: {{base_endpoint_url}}
- OpenAPI spec: {{openapi_url}}
- Bootstrap guide: {{guide_url}}
- Global event stream: {{events_stream_url}}
- WebSocket base: {{websocket_base_url}}
- Always authenticate with: Authorization: Bearer <OPENCLAW_API_KEY>
- Use only the OpenClaw gateway namespaces:
  - REST/SSE: /api/openclaw/v1/*
  - WebSocket: /api/openclaw/ws/*
- Do not call internal ACPMS routes such as /api/v1/* or /ws/* directly unless ACPMS documentation explicitly allows it.

Discovery workflow:
1. Load this guide into your ACPMS integration context.
2. Fetch and parse the ACPMS OpenAPI contract.
3. Build your ACPMS tool surface dynamically from that contract.
4. Open and maintain the ACPMS global event stream connection.
5. If optional Webhook delivery is intentionally enabled, provide or confirm your webhook receiver URL.
6. Verify that your reporting channels for the primary user are configured.

Webhook verification:
- ACPMS signs outgoing webhooks with HMAC-SHA256.
- Signature header: X-Agentic-Signature
- Verify the raw HTTP body exactly as received.
- Use the provided OPENCLAW_WEBHOOK_SECRET to validate signatures when optional Webhook delivery is enabled.
- Reject webhook payloads if verification fails.

Operational behavior:
- Treat ACPMS as the source of truth for state.
- Prefer the global event stream and attempt SSE/WebSocket streams for long-running visibility.
- Use optional Webhooks only when the deployment has explicitly enabled them.
- Use mirrored ACPMS APIs for all create/read/update/control actions.
- Classify each user command before acting: status/reporting, requirement analysis, work creation, execution, investigation, control, or admin/system.
- For status/reporting commands, read ACPMS and report without mutating ACPMS.
- For requirement-analysis commands, load ACPMS context first, detect overlap/conflicts, and propose a solution before making changes.
- For work-creation commands, check whether equivalent requirements/tasks already exist before creating new ones.
- For execution commands, ensure the target work exists, then start and monitor attempts, and report lifecycle changes.
- Re-read ACPMS state before retrying state-conflict operations.
- Before proposing a solution to the user, gather the relevant ACPMS context needed to avoid duplicate or conflicting work.
- When the user gives a new requirement, analyze it against ACPMS context first, then propose a concrete solution or execution plan.
- When the solution is approved, create or update ACPMS entities and run the relevant task attempts.
- On authentication failure, stop and alert the operator.
- On gateway-disabled or blocked responses, stop retrying and report the issue.
- On overload or server failures, retry with bounded exponential backoff when safe.

Human reporting behavior:
- Your primary human-facing responsibility is to keep the user informed.
- Report important events and summaries to the primary user through configured channels such as Telegram or Slack.
- For every material ACPMS-related response, report:
  - what the user asked
  - what ACPMS context you checked
  - what you concluded
  - what ACPMS action you took, if any
  - current status
  - next step or needed human approval
- Send concise reports for:
  - requirement analysis and recommended solution
  - attempt started
  - attempt completed
  - attempt failed
  - approval required
  - deployment risk or production-impacting changes
  - health/integration incidents
- Summaries should include:
  - what ACPMS entity was affected
  - current status
  - what changed
  - what action is needed from the human, if any
- Do not send secrets, API keys, webhook secrets, or sensitive credentials to user-facing channels.

Preferred first actions:
- Load OpenAPI.
- Open the global event stream.
- List projects.
- Check settings and integration status.
- Check webhook registration status only if optional Webhook delivery is enabled.
- Validate your reporting channel configuration.
- Then proceed with normal ACPMS operations.
```

### 5.4 Operating Rulebook Semantics

OpenClaw should treat the ACPMS rulebook as an execution policy layer on top of the API surface.

At minimum, the bootstrap response should tell OpenClaw:

1.  the default autonomy mode
2.  whether ACPMS context must be loaded before mutation
3.  whether material changes must always be reported
4.  which events require immediate user notification
5.  whether destructive actions always require confirmation

The detailed behavioral rulebook is described in:

*   `docs/plan-feature/openclaw-gateway/11_operating_rules.md`

### 5.2 Reporting Semantics

OpenClaw should treat itself as a **bi-directional operations assistant**:

*   ACPMS -> OpenClaw: source of truth, execution state, alerts, and control plane
*   OpenClaw -> Primary User: operational summaries, requirement analysis, proposed solutions, approval requests, failure alerts, and status reporting via Telegram, Slack, or other configured channels

### 5.3 Requirement-Analysis Semantics

When the primary user provides a new requirement, OpenClaw should:

1.  load the relevant ACPMS context first
2.  identify whether the requirement overlaps with existing requirements, tasks, or active attempts
3.  produce a solution recommendation or execution plan
4.  report that recommendation to the primary user
5.  after approval, create or update ACPMS requirements/tasks and start execution where appropriate

## 6. Connection Finalization Behavior

If `webhook_receiver_url` is included in the bootstrap request:

1.  ACPMS validates the URL format.
2.  ACPMS stores it in OpenClaw integration settings/state.
3.  ACPMS marks the Webhook direction as configured.
4.  The response reports `webhook_registered: true`.

If the URL is missing:

*   ACPMS still returns the guide.
*   The response should not treat this as an error when the deployment is using the default stream-first integration model.
*   The response includes `missing_steps` only if the operator explicitly requested optional Webhook delivery but did not finish configuring it.

If reporting channel metadata is included in the bootstrap request:

1.  ACPMS may echo that information back in `reporting_policy`.
2.  OpenClaw uses it to decide where human-facing reports should be sent.
3.  If no reporting channels are configured, `missing_steps` should instruct OpenClaw to request operator setup for Telegram, Slack, or an equivalent channel.

## 7. Why `POST` Instead of `GET`

The endpoint is intentionally `POST` even though it returns a guide, because:

*   it may persist OpenClaw connection metadata
*   it may register the Webhook receiver URL as part of bootstrap
*   it may capture reporting-channel preferences for the primary user
*   it is conceptually a **handshake/bootstrap action**, not just a static document fetch

## 8. Installer Output

When OpenClaw Gateway is enabled, `install.sh` should print:

*   `Base Endpoint URL`
*   `OpenAPI (Swagger)`
*   `Guide Endpoint`
*   `Global Event SSE`
*   `API Key (Bearer)`
*   `Webhook Secret` (optional)
*   `Ready-to-Send Installer Prompt`

The installer-generated handoff prompt is defined in:

*   `docs/plan-feature/openclaw-gateway/12_installer_bootstrap_prompt.md`

OpenClaw then uses these values in this order:

1.  Receive the installer-generated prompt from the human user
2.  Call `Guide Endpoint`
3.  Load `OpenAPI (Swagger)`
4.  Open `Global Event SSE`
5.  Store `Webhook Secret` only if optional Webhook mode is enabled
6.  Configure reporting to the primary user via Telegram, Slack, or another supported OpenClaw channel
7.  Start using the mirrored ACPMS admin API surface
