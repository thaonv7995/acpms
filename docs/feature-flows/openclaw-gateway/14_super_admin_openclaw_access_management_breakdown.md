# OpenClaw Gateway: 14 - Super Admin Settings Access Management Breakdown

## 1. Purpose

This document breaks down the `Super Admin Settings -> OpenClaw Access Management` feature into implementation-sized units.

It focuses on one operator-facing slice of the larger auth upgrade:

* viewing enrolled OpenClaw clients
* generating a ready-to-send bootstrap prompt with a single-use bootstrap token
* disabling or enabling one OpenClaw client without affecting others

This document is intentionally narrower than the main auth-upgrade design doc. It exists so engineering can implement the settings-side management surface with concrete UI, API, and testing steps.

Related design:

* `13_auth_upgrade_bootstrap_tokens_and_asymmetric_client_proofs.md`

## 2. Feature Summary

The `SettingsPage` should expose an `OpenClaw Access` management entry point for super admins.

When the admin clicks it:

* ACPMS opens a modal or popup
* the modal lists enrolled OpenClaw clients
* the modal allows the admin to generate a new bootstrap prompt for another OpenClaw installation
* the modal allows the admin to disable or re-enable any enrolled OpenClaw client

The modal is not the place for runtime secrets. Its main responsibilities are lifecycle management and controlled enrollment.

## 3. High-Level UX

### 3.1 Entry Point

Screen:

* `frontend/src/pages/SettingsPage.tsx`

Entry:

* add a new section in the global system settings area for `OpenClaw Access`
* include a button such as `Manage OpenClaw`

Permissions:

* visible only to system admins
* uses the same system-admin permission model already applied to settings APIs

### 3.2 Modal Structure

Recommended modal sections:

1. `Clients`
2. `Add OpenClaw`
3. `Prompt Output`

Recommended behavior:

* open with client list loaded
* keep create-bootstrap form on the same modal to reduce navigation
* show generated prompt only after successful creation

### 3.3 Client Row Actions

Per-client actions:

* `Disable Access`
* `Enable Access`
* `View Details`
* optional `Revoke`

Semantics:

* `Disable` is reversible
* `Enable` reactivates the enrolled client
* `Revoke` is stronger than disable and should require a separate confirmation

## 4. Scope

### 4.1 In Scope

* Settings page entry point
* OpenClaw management modal
* Admin APIs for list/create prompt/disable/enable
* Rendering a single-use bootstrap prompt
* Displaying enrolled clients and their status
* Success and error handling in the modal
* Unit and integration tests for the settings slice

### 4.2 Out of Scope

* Full bootstrap protocol implementation details
* Runtime signature verification middleware
* Client-side key rotation UX
* Webhook UX
* Non-admin entry points

## 5. Proposed Files

### 5.1 Frontend

Existing files to update:

* `frontend/src/pages/SettingsPage.tsx`
* `frontend/src/api/settings.ts`

Recommended new files:

* `frontend/src/components/modals/OpenClawAccessModal.tsx`
* `frontend/src/hooks/useOpenClawAccess.ts`
* `frontend/src/__tests__/unit/components/OpenClawAccessModal.test.tsx`
* `frontend/src/__tests__/unit/pages/SettingsPage.openclaw-access.test.tsx`

### 5.2 Backend

Existing files to update:

* `crates/server/src/routes/settings.rs`
* `crates/server/src/routes/mod.rs` only if route registration must be split or reorganized

Recommended backend additions:

* request/response DTOs in `crates/server/src/routes/settings.rs` or a new route module if the file grows too large
* service-layer support in `crates/services` for bootstrap token issuance and client state updates

## 6. Backend API Surface

This settings feature should depend on internal admin APIs under `/api/v1`, not `/api/openclaw/*`.

### 6.1 List Clients

Method:

* `GET /api/v1/admin/openclaw/clients`

Purpose:

* populate the modal client list

Recommended response shape:

```json
{
  "success": true,
  "code": "0000",
  "message": "OpenClaw clients retrieved successfully",
  "data": {
    "clients": [
      {
        "client_id": "oc_client_01HV...",
        "display_name": "OpenClaw Production",
        "status": "active",
        "enrolled_at": "2026-03-10T09:00:00Z",
        "last_seen_at": "2026-03-10T09:30:00Z",
        "last_seen_ip": "203.0.113.10",
        "last_seen_user_agent": "OpenClaw/1.0.0",
        "key_fingerprints": [
          "ed25519:ab12cd34"
        ]
      }
    ]
  }
}
```

### 6.2 Create Bootstrap Prompt

Method:

* `POST /api/v1/admin/openclaw/bootstrap-tokens`

Purpose:

* create one single-use bootstrap token
* return a prompt ready to send to another OpenClaw installation

Recommended request:

```json
{
  "label": "OpenClaw Staging",
  "expires_in_minutes": 15,
  "suggested_display_name": "OpenClaw Staging",
  "metadata": {
    "environment": "staging"
  }
}
```

Recommended response:

```json
{
  "success": true,
  "code": "0000",
  "message": "Bootstrap prompt generated successfully",
  "data": {
    "bootstrap_token_id": "uuid",
    "expires_at": "2026-03-10T10:15:00Z",
    "prompt_text": "You are being connected to an ACPMS instance ...",
    "token_preview": "oc_boot_****"
  }
}
```

Rules:

* raw bootstrap token must be present only in `prompt_text`
* raw token should not be returned later by any list endpoint

### 6.3 Disable Client

Method:

* `POST /api/v1/admin/openclaw/clients/{client_id}/disable`

Purpose:

* temporarily block runtime access for one OpenClaw client

Result:

* client status becomes `disabled`

### 6.4 Enable Client

Method:

* `POST /api/v1/admin/openclaw/clients/{client_id}/enable`

Purpose:

* restore runtime access for one disabled OpenClaw client

Result:

* client status becomes `active`

### 6.5 Optional Revoke

Method:

* `POST /api/v1/admin/openclaw/clients/{client_id}/revoke`

Purpose:

* permanently revoke a client

This is optional for the first slice if the goal is only enable/disable plus add-new-client.

## 7. Backend Behavior Rules

### 7.1 Authorization

All settings-side OpenClaw management endpoints must:

* require normal ACPMS user JWT auth
* require system-admin permission

They must not accept:

* bootstrap tokens
* OpenClaw runtime signed auth

### 7.2 Status Rules

Recommended allowed transitions:

* `active -> disabled`
* `disabled -> active`
* `active -> revoked`
* `disabled -> revoked`

Recommended forbidden transitions:

* `revoked -> active`
* `revoked -> disabled`

### 7.3 Prompt Generation Rules

When the admin generates a prompt:

* issue exactly one bootstrap token
* bind TTL and label
* build the prompt from current ACPMS base URLs
* include guide/openapi/events/ws endpoints
* clearly state that the token is single-use and expires

### 7.4 Audit Rules

Every admin action should be auditable:

* client list viewed
* bootstrap token created
* client disabled
* client enabled
* client revoked

Minimum audit fields:

* acting admin user ID
* target client ID or token ID
* action type
* timestamp

## 8. Frontend Breakdown

### 8.1 Settings Page Changes

File:

* `frontend/src/pages/SettingsPage.tsx`

Tasks:

* add local state for `showOpenClawAccessDialog`
* add a section header such as `OpenClaw Access`
* add button `Manage OpenClaw`
* render `OpenClawAccessModal`

### 8.2 API Layer Changes

File:

* `frontend/src/api/settings.ts`

Add types and functions for:

* `getOpenClawClients()`
* `createOpenClawBootstrapPrompt()`
* `disableOpenClawClient(clientId)`
* `enableOpenClawClient(clientId)`
* optional `revokeOpenClawClient(clientId)`

Recommended types:

* `OpenClawClientDto`
* `OpenClawClientsResponse`
* `CreateOpenClawBootstrapPromptRequest`
* `CreateOpenClawBootstrapPromptResponse`

### 8.3 Hook Changes

Recommended new hook:

* `frontend/src/hooks/useOpenClawAccess.ts`

Responsibilities:

* fetch client list
* expose loading and mutation states
* refresh after create/disable/enable actions
* centralize toast-friendly error messages

### 8.4 Modal Changes

Recommended new component:

* `frontend/src/components/modals/OpenClawAccessModal.tsx`

Responsibilities:

* render enrolled client list
* render create-bootstrap form
* render prompt output block after successful creation
* handle loading, empty state, and action buttons

Recommended modal states:

* `idle`
* `loading_clients`
* `creating_prompt`
* `action_in_progress`
* `error`

### 8.5 Prompt Output UX

Recommended behavior:

* after successful create, show generated prompt in a read-only text area
* provide `Copy Prompt` button
* show expiry timestamp clearly
* show warning that the raw bootstrap token is only available in this prompt output view

## 9. Detailed Implementation Checklist

### 9.1 Backend

- [ ] Add DB tables for `openclaw_bootstrap_tokens`, `openclaw_clients`, and `openclaw_client_keys` if they do not already exist.
- [ ] Add server-side DTOs for OpenClaw admin settings responses.
- [ ] Add `GET /api/v1/admin/openclaw/clients`.
- [ ] Add `POST /api/v1/admin/openclaw/bootstrap-tokens`.
- [ ] Add `POST /api/v1/admin/openclaw/clients/{client_id}/disable`.
- [ ] Add `POST /api/v1/admin/openclaw/clients/{client_id}/enable`.
- [ ] Enforce `system admin` permission on all new admin endpoints.
- [ ] Add audit logging for list/create/disable/enable actions.
- [ ] Return prompt text only at bootstrap-token creation time.

### 9.2 Frontend

- [ ] Add OpenClaw management API functions in `frontend/src/api/settings.ts`.
- [ ] Add `useOpenClawAccess` hook for query and mutation state.
- [ ] Create `OpenClawAccessModal.tsx`.
- [ ] Add `Manage OpenClaw` button to `SettingsPage.tsx`.
- [ ] Wire modal open/close state in `SettingsPage.tsx`.
- [ ] Render enrolled client table with `status`, `client_id`, `enrolled_at`, `last_seen_at`, and actions.
- [ ] Render create-bootstrap form with label and TTL.
- [ ] Render prompt output with copy action and expiry warning.
- [ ] Disable row action buttons while mutations are in flight.

### 9.3 UX and Copy

- [ ] Use clear operator-facing labels: `Manage OpenClaw`, `Add OpenClaw`, `Disable Access`, `Enable Access`.
- [ ] Show a warning that generated bootstrap prompts are sensitive and time-limited.
- [ ] Show empty state text when no OpenClaw clients exist yet.
- [ ] Show a distinct badge for `active`, `disabled`, and `revoked`.

### 9.4 Validation

- [ ] Verify a system admin can open the modal and load an empty client list.
- [ ] Verify creating a bootstrap prompt returns prompt text and expiry.
- [ ] Verify copying the prompt works.
- [ ] Verify disabling one client changes only that client to `disabled`.
- [ ] Verify enabling one disabled client changes only that client back to `active`.
- [ ] Verify non-admin users cannot access the admin endpoints.

## 10. Acceptance Criteria

This settings slice is complete when:

* the `SettingsPage` shows a `Manage OpenClaw` entry for super admins
* clicking the entry opens a modal with enrolled OpenClaw clients
* the modal can generate a new bootstrap prompt with a single-use token
* the modal can disable one client without affecting other clients
* the modal can re-enable a disabled client
* the raw bootstrap token is not returned again after prompt creation
* all actions are backed by admin-only `/api/v1/admin/openclaw/*` endpoints

## 11. Recommended Delivery Order

1. Add backend list/create/disable/enable endpoints.
2. Add frontend API client functions and hook.
3. Create `OpenClawAccessModal`.
4. Add `Manage OpenClaw` entry to `SettingsPage`.
5. Add tests for modal behavior and API access control.
6. Hook the settings doc and auth-upgrade doc to the new implementation slice.

## 12. Notes for Follow-Up

After this settings slice lands, the next logical implementation docs should cover:

* runtime signed-auth middleware
* bootstrap enrollment endpoints
* OpenClaw-side prompt consumption and key generation
* client details view and key rotation UX
