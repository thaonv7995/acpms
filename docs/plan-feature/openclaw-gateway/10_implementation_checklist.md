# OpenClaw Gateway: 10 - Implementation Checklist

## 1. Goal

This checklist turns the stream-first OpenClaw design into an implementation plan that can actually ship.

The immediate target is:

*   `install.sh` generates a copy-paste OpenClaw bootstrap prompt for the human operator.
*   OpenClaw connects to ACPMS without exposing any inbound domain.
*   OpenClaw can bootstrap from the installer prompt without the human manually explaining ACPMS.
*   OpenClaw receives lifecycle events through `GET /api/openclaw/v1/events/stream`.
*   OpenClaw can reconnect and recover missed terminal events by cursor.
*   Attempt-specific logs remain available through `GET /api/openclaw/v1/attempts/{attempt_id}/stream`.
*   Optional Webhooks remain additive and do not block the default integration.

## 2. MVP Success Criteria

The MVP is complete only when all of the following are true:

*   `install.sh` prints a ready-to-send OpenClaw prompt and the same prompt can be saved to a local file.
*   OpenClaw can read that prompt and know that its first authoritative action is `POST /api/openclaw/guide-for-openclaw`.
*   The bootstrap response includes `events_stream_url`, `operating_rules`, and reporting-policy guidance.
*   OpenClaw can create a task attempt and receive `attempt.started`.
*   OpenClaw can observe `attempt.completed`, `attempt.failed`, or `attempt.needs_input` on the global event stream.
*   OpenClaw can disconnect, reconnect with `Last-Event-ID` or `?after=<cursor>`, and recover missed events.
*   OpenClaw can confirm final state by calling `GET /api/openclaw/v1/attempts/{id}` after a terminal event.
*   Missing Webhook configuration does not degrade the baseline ACPMS <-> OpenClaw connection.

## 3. Scope and Non-Goals

### 3.1 In Scope

*   Gateway auth for the global event stream
*   Installer-generated bootstrap prompt
*   Bootstrap guide response contract
*   Structured operating-rules payload for OpenClaw behavior
*   Canonical OpenClaw event model
*   Durable replay store in the main Postgres database
*   Replay cursor semantics
*   Global SSE event endpoint
*   Attempt log SSE reuse
*   Event emission from task/attempt lifecycle code paths
*   Bootstrap/OpenAPI/docs updates
*   Tests for auth, replay, reconnection, and terminal lifecycle delivery

### 3.2 Out of Scope for MVP

*   Kafka, NATS, Redis Streams, or another external message bus
*   Exactly-once delivery guarantees
*   Per-subscriber delivery tracking in the database
*   Multi-tenant event isolation beyond the existing OpenClaw gateway auth model
*   Rich event filtering syntax beyond basic cursor resume

## 4. Recommended Implementation Order

### 4.0 Phase 0: Human Handoff Contract

- [ ] Keep `install.sh` prompt default disabled with `Do you want to enable the OpenClaw Integration Gateway for external AI control? [y/N]`.
- [ ] When enabled, generate:
  - `OPENCLAW_API_KEY`
  - `OPENCLAW_WEBHOOK_SECRET` (optional transport support)
  - a rendered OpenClaw bootstrap prompt
- [ ] Print both:
  - operator reference connection details
  - a single ready-to-send prompt block for OpenClaw
- [ ] Optionally save the rendered prompt to `~/.acpms/config/openclaw_bootstrap_prompt.txt`.
- [ ] If saved to disk, create the file with restrictive permissions where practical.
- [ ] Ensure the installer prompt tells OpenClaw to call `POST /api/openclaw/guide-for-openclaw` first.
- [ ] Ensure the installer prompt tells OpenClaw to use only `/api/openclaw/v1/*` and `/api/openclaw/ws/*`.

### 4.1 Phase 1: Define the Canonical Event Contract

- [ ] Create a shared event model for OpenClaw gateway events.
- [ ] Keep event names stable and explicit:
  - `attempt.started`
  - `attempt.completed`
  - `attempt.failed`
  - `attempt.needs_input`
  - `attempt.cancelled` if ACPMS exposes cancel as a first-class terminal state
  - `task.status_changed`
  - `approval.required` if approval queues are already implemented
  - `system.alert` only for high-signal operational failures
- [ ] Distinguish **task state** from **attempt state** in the payload and in internal comments/docs.
- [ ] Define one envelope shape for all stream events.

Recommended event envelope:

```json
{
  "id": "12345",
  "type": "attempt.completed",
  "occurred_at": "2026-03-08T10:15:00Z",
  "project_id": "uuid-or-null",
  "task_id": "uuid-or-null",
  "attempt_id": "uuid-or-null",
  "payload": {
    "status": "success",
    "summary": "Completed implementation"
  }
}
```

Required rules:

*   `id` must be monotonic for replay and resume.
*   `type` must be stable across releases.
*   `occurred_at` must be UTC.
*   `payload` must not include secrets, tokens, webhook secrets, or raw credentials.
*   Terminal attempt events must include enough identifiers for OpenClaw to immediately re-read the authoritative ACPMS resource.

### 4.2 Phase 2: Add a Durable Replay Store

- [ ] Implement the replay store in the same Postgres database ACPMS already uses.
- [ ] Add a migration for a dedicated event table.
- [ ] Use an append-only write pattern.
- [ ] Use a monotonic numeric cursor as the replay anchor.

Recommended table:

```sql
CREATE TABLE openclaw_gateway_events (
    sequence_id BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    project_id UUID NULL,
    task_id UUID NULL,
    attempt_id UUID NULL,
    source TEXT NOT NULL,
    payload JSONB NOT NULL
);
```

Recommended indexes:

```sql
CREATE INDEX idx_openclaw_events_occurred_at
    ON openclaw_gateway_events (occurred_at DESC);

CREATE INDEX idx_openclaw_events_attempt_id
    ON openclaw_gateway_events (attempt_id, sequence_id DESC);

CREATE INDEX idx_openclaw_events_task_id
    ON openclaw_gateway_events (task_id, sequence_id DESC);

CREATE INDEX idx_openclaw_events_project_id
    ON openclaw_gateway_events (project_id, sequence_id DESC);
```

Required write semantics:

*   Persist the event before publishing it to the in-memory broadcast fan-out.
*   For DB-backed state transitions, prefer writing the event in the same transaction as the business-state update whenever practical.
*   For runtime-only transitions, persist the event immediately after the state change and before notifying external clients.
*   Do not make OpenClaw replay depend on in-memory-only buffers.

Retention:

- [ ] Add a retention policy, for example `OPENCLAW_EVENT_RETENTION_HOURS=168`.
- [ ] Add a cleanup job that deletes expired rows in bounded batches.
- [ ] Document behavior when a client asks for a cursor older than the retained window.

### 4.3 Phase 3: Build the Event Service Layer

- [ ] Add a small service responsible for:
  - inserting event rows
  - converting DB rows to SSE payloads
  - reading replay ranges after a cursor
  - publishing live events to a `broadcast::Sender`
- [ ] Keep this service separate from HTTP route code.
- [ ] Reuse the service from both the global SSE endpoint and optional Webhook dispatch logic.

Required service methods:

*   `record_event(event_type, refs..., payload) -> sequence_id`
*   `list_events_after(sequence_id, limit) -> Vec<Event>`
*   `subscribe_live() -> broadcast::Receiver<Event>`
*   `cleanup_expired_events(retention_cutoff)`

### 4.4 Phase 4: Implement `GET /api/openclaw/v1/events/stream`

- [ ] Add the Axum route under the OpenClaw gateway namespace.
- [ ] Guard it with the same OpenClaw bearer auth as other gateway APIs.
- [ ] Accept `Last-Event-ID` and `?after=<cursor>`.
- [ ] Replay missed events first, then switch to live subscription.
- [ ] Send heartbeats with `KeepAlive`.
- [ ] Emit SSE `id:` from `sequence_id`.
- [ ] Emit SSE `event:` from the canonical `event_type`.
- [ ] Emit SSE `data:` as JSON.

Required behavior:

*   If no cursor is provided, start streaming from "now" rather than replaying the entire retained history.
*   If both `Last-Event-ID` and `after` are provided, reject the request as ambiguous.
*   If the cursor is malformed, return `400 Bad Request`.
*   If the cursor points to data older than the retention window, return `409 Conflict` with a clear machine-readable error code such as `4091` / `EventCursorExpired`.
*   If the gateway is disabled, return `403 Forbidden`.

### 4.5 Phase 5: Wire Real Event Producers

- [ ] Emit `attempt.started` when an attempt genuinely enters running state.
- [ ] Emit `attempt.completed` only after ACPMS has finalized terminal success state and summary metadata.
- [ ] Emit `attempt.failed` only after ACPMS has finalized terminal failure state.
- [ ] Emit `attempt.needs_input` when ACPMS pauses execution and exposes the prompt to OpenClaw.
- [ ] Emit `task.status_changed` when the persisted task column/status actually changes.
- [ ] Emit `attempt.cancelled` if cancellation is distinguishable from generic failure in ACPMS.

Primary integration points to wire:

*   orchestrator lifecycle transitions
*   task status mutations
*   approval/HITL pause points
*   deploy/review flows only after the attempt/task path is stable

Ordering requirements:

*   If a task attempt is created and then started, OpenClaw must see the start event after the attempt exists.
*   If a terminal event is emitted, the corresponding `GET /attempts/{id}` call must already reflect that final state.
*   `task.status_changed` should be emitted after the task row is committed.

### 4.6 Phase 6: Keep Attempt Log Streams Separate

- [ ] Leave `GET /api/openclaw/v1/attempts/{attempt_id}/stream` focused on logs and low-level execution output.
- [ ] Do not overload the attempt log stream to be the sole source of lifecycle truth.
- [ ] Keep the global event stream as the system-of-notification channel.

OpenClaw should use:

*   global stream for lifecycle and business events
*   attempt stream for detailed logs
*   `GET /attempts/{id}` and `GET /tasks/{id}` for final confirmation

### 4.7 Phase 7: Update Bootstrap and OpenAPI Surface

- [ ] Add `events_stream_url` to the bootstrap response.
- [ ] Add `operating_rules` to the bootstrap response.
- [ ] Add reporting-policy fields that tell OpenClaw what must be reported to the user.
- [ ] Keep the bootstrap response consistent with the installer-generated prompt.
- [ ] Add resume guidance to `instruction_prompt`.
- [ ] Document `GET /api/openclaw/v1/events/stream` in OpenAPI and Swagger UI.
- [ ] Keep Webhook fields optional, not required.
- [ ] Ensure installer output includes `Global Event SSE`.

### 4.8 Phase 8: Implement ACPMS Operating-Rule Contract

- [ ] Return rulebook metadata such as:
  - `rulebook_version`
  - `default_autonomy_mode`
  - `must_load_acpms_context_before_mutation`
  - `must_report_material_changes`
  - `must_confirm_before_destructive_actions`
  - `high_priority_report_events`
  - `recommended_reporting_template`
- [ ] Keep the bootstrap response aligned with the OpenClaw operating-rule doc.
- [ ] Ensure the rule payload distinguishes:
  - read/report-only actions
  - analysis/proposal actions
  - work-creation actions
  - execution actions
  - control/admin actions
- [ ] Ensure the rule payload makes OpenClaw report:
  - what the user asked
  - what ACPMS context was checked
  - what was concluded
  - what ACPMS action was taken, if any
  - current status
  - next step / approval needed

### 4.9 Phase 9: Optional Webhook Compatibility

- [ ] Keep the event payload schema identical between SSE and Webhook delivery where possible.
- [ ] Reuse the same event service as the source for Webhook dispatch.
- [ ] Do not require `webhook_receiver_url` for bootstrap success.
- [ ] Report `webhook_registered=false` without treating it as a setup failure in stream-first mode.

### 4.10 Phase 10: Installer Prompt Rendering

- [ ] Render the installer prompt from the same canonical field set used by the bootstrap response.
- [ ] Avoid hand-maintaining two divergent prompt templates.
- [ ] Ensure the prompt includes:
  - Base endpoint
  - OpenAPI URL
  - Guide endpoint
  - Global Event SSE URL
  - API key
  - optional webhook secret
  - first-step instructions
  - human reporting obligations
- [ ] Ensure the prompt is concise enough to paste into OpenClaw directly.
- [ ] Ensure the prompt does not inline the entire long-form rulebook.

## 5. Cursor and Replay Rules

These rules should be implemented exactly to avoid ambiguous client behavior.

### 5.1 Cursor Format

- [ ] Use `sequence_id` as the canonical replay cursor.
- [ ] Serialize it as a string in SSE `id:` so it is safe for `Last-Event-ID`.
- [ ] Document that the cursor is opaque from the client point of view even if it is numerically ordered.

### 5.2 Initial Connection

- [ ] If OpenClaw connects with no cursor, start from live events only.
- [ ] OpenClaw should separately call ACPMS list/read APIs after bootstrap to establish current state.

### 5.3 Reconnection

- [ ] If OpenClaw reconnects with `Last-Event-ID=12345`, replay events where `sequence_id > 12345`.
- [ ] After replay completes, continue with live subscription on the same HTTP response.
- [ ] Ensure replay and live fan-out do not duplicate the same event in one reconnect flow.

### 5.4 Retention Failure

- [ ] If the earliest retained row is newer than the requested cursor, return a structured conflict error.
- [ ] The error should instruct OpenClaw to resync by re-reading ACPMS state and then reopening the stream without a stale cursor.

## 6. Security and Auditing Checklist

- [ ] Reuse `Authorization: Bearer <OPENCLAW_API_KEY>` for the event stream.
- [ ] Audit stream connect/disconnect events with request metadata.
- [ ] Redact sensitive fields before event persistence.
- [ ] Ensure event payloads do not leak raw process environment, access tokens, or secret values.
- [ ] Rate-limit stream connection churn if needed, but do not rate-limit a healthy long-lived connection aggressively.

## 7. Testing Checklist

### 7.1 Unit Tests

- [ ] Event serialization/deserialization
- [ ] Cursor parsing and validation
- [ ] Replay query ordering
- [ ] Retention cutoff cleanup
- [ ] Installer prompt rendering from runtime config
- [ ] Bootstrap response serialization including `operating_rules`

### 7.2 Integration Tests

- [ ] Unauthorized stream request returns `401`
- [ ] Disabled gateway returns `403`
- [ ] `guide-for-openclaw` returns required runtime fields for stream-first mode
- [ ] Installer-generated prompt and bootstrap response stay field-consistent
- [ ] Connect with no cursor and receive live event
- [ ] Reconnect with `Last-Event-ID` and receive missed events
- [ ] Expired cursor returns structured conflict error
- [ ] Attempt start -> completion emits expected global events in order
- [ ] `attempt.needs_input` appears on the global stream and can be resolved via `POST /attempts/{id}/input`
- [ ] Attempt log stream still works independently from the global event stream

### 7.3 Manual Verification

- [ ] Start ACPMS locally with OpenClaw gateway enabled
- [ ] Verify installer prints a ready-to-send OpenClaw prompt
- [ ] Verify prompt file is created when that behavior is enabled
- [ ] Paste the installer prompt into OpenClaw and verify it calls `Guide Endpoint` first
- [ ] Open one terminal with `curl -N -H "Authorization: Bearer ..."` against `/api/openclaw/v1/events/stream`
- [ ] Create and run a task attempt
- [ ] Verify `attempt.started` arrives
- [ ] Force success, failure, and needs-input scenarios
- [ ] Disconnect and reconnect using `Last-Event-ID`
- [ ] Verify no inbound OpenClaw domain is needed

## 8. Operational Checklist

- [ ] Add metrics:
  - active OpenClaw event stream connections
  - replayed event count
  - retained event row count
  - replay cursor expired count
- [ ] Add structured logs for stream open, replay start, replay end, disconnect, and auth failure.
- [ ] Add a retention cleanup schedule.
- [ ] Add a feature flag only if rollout needs staged enablement beyond `OPENCLAW_GATEWAY_ENABLED`.

## 9. Implementation Task Breakdown

### 9.1 Database

- [ ] Migration for `openclaw_gateway_events`
- [ ] Retention cleanup query
- [ ] Tests for replay ordering

### 9.2 Services

- [ ] `OpenClawEventService`
- [ ] Event row DTO + domain DTO
- [ ] Broadcast integration
- [ ] Bootstrap-guide builder / serializer
- [ ] Installer-prompt renderer from canonical runtime config

### 9.3 Server Routes

- [ ] `routes/openclaw/events.rs`
- [ ] Router registration in `routes/openclaw/mod.rs`
- [ ] OpenAPI annotations
- [ ] `routes/openclaw/guide.rs` returns `operating_rules` and transport metadata

### 9.4 Event Producers

- [ ] Orchestrator lifecycle hooks
- [ ] Task service status hooks
- [ ] HITL hooks
- [ ] Optional deployment/review hooks

### 9.5 Docs and Installer

- [ ] Keep `/api/openclaw/guide-for-openclaw` aligned with the final stream endpoint
- [ ] Keep `/api/openclaw/openapi.json` aligned with runtime behavior
- [ ] Update `install.sh` output when implementation lands
- [ ] Update `install.sh` to print the ready-to-send prompt block
- [ ] Update `install.sh` to save the rendered prompt to a file if that behavior is enabled
- [ ] Keep installer prompt content aligned with the OpenClaw operating rules and guide contract

## 10. Recommended Shipping Sequence

Ship this in five PRs if possible:

1.  **PR 1**: installer prompt renderer + guide-response contract + shared config fields
2.  **PR 2**: event model + DB migration + event service
3.  **PR 3**: `GET /api/openclaw/v1/events/stream` + replay logic + tests
4.  **PR 4**: orchestrator/task/HITL event emission wiring
5.  **PR 5**: bootstrap/OpenAPI/install updates + optional Webhook compatibility cleanup

This sequence keeps the work reviewable and reduces the chance of mixing transport design, persistence, and orchestration bugs in one large change.
