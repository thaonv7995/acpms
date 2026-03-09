# OpenClaw Gateway: 12 - Installer Bootstrap Prompt

## 1. Purpose

When `install.sh` enables the OpenClaw gateway, it should not only print raw credentials.

It should also print a **ready-to-send bootstrap prompt** so the human user can copy one whole block and send it directly to OpenClaw.

The intended outcome is:

*   the user does not need to manually explain ACPMS to OpenClaw
*   the user does not need to manually map each field one by one
*   OpenClaw receives enough context to bootstrap itself correctly
*   OpenClaw knows that its first authoritative action is to call `GET /api/openclaw/guide-for-openclaw` for basic bootstrap

## 2. Output Contract

`install.sh` should produce two OpenClaw-related outputs:

1.  a **connection details section** for human reference
2.  a **ready-to-send prompt block** for direct handoff to OpenClaw

Optionally, `install.sh` should also save that prompt to a local file such as:

*   `~/.acpms/config/openclaw_bootstrap_prompt.txt`

This file path is only a convenience mechanism. The critical requirement is that the user can copy one complete prompt block and send it to OpenClaw without extra editing.

## 3. Required Installer Prompt Content

The installer-generated prompt must include:

*   product identity: ACPMS / Agentic Coding Project Management System
*   role identity: OpenClaw acts as a trusted Super Admin integration and operations assistant
*   instance-specific connection bundle:
  - `Base Endpoint URL`
  - `OpenAPI (Swagger)`
  - `Guide Endpoint`
  - `Global Event SSE`
  - `API Key (Bearer)`
  - `Webhook Secret` if optional Webhook mode is enabled
*   the rule that OpenClaw must call `Guide Endpoint` first
*   the rule that OpenClaw must use only `/api/openclaw/v1/*` and `/api/openclaw/ws/*`
*   the rule that OpenClaw must treat ACPMS as the source of truth
*   the rule that OpenClaw must report important status, actions, failures, and approvals back to the primary user
*   the rule that OpenClaw must not expose secrets in human-facing messages

## 4. Recommended Installer UX

The OpenClaw section of `install.sh` output should look like this:

```text
================================================================================
 OPENCLAW GATEWAY CONFIGURATION
================================================================================
 Base Endpoint URL : https://api.yourdomain.com/api/openclaw/v1
 OpenAPI (Swagger) : https://api.yourdomain.com/api/openclaw/openapi.json
 Guide Endpoint    : https://api.yourdomain.com/api/openclaw/guide-for-openclaw
 Global Event SSE  : https://api.yourdomain.com/api/openclaw/v1/events/stream
 API Key (Bearer)  : oc_live_5x8a9b2c3d4e5f6g7h8i9j0k
 Webhook Secret    : wh_sec_a1b2c3d4e5f6g7h8i9j0k1l2 (optional)
 Prompt File       : ~/.acpms/config/openclaw_bootstrap_prompt.txt
================================================================================

================================================================================
 OPENCLAW READY-TO-SEND PROMPT
================================================================================
 Copy everything below and send it to OpenClaw:

You are being connected to an ACPMS (Agentic Coding Project Management System) instance.

Your role for this ACPMS instance:
- act as a trusted Super Admin integration
- act as an operations assistant for the primary user
- load ACPMS context before making decisions
- analyze requirements using ACPMS data
- create/update ACPMS work only when requested or allowed by autonomy policy
- monitor running attempts and report meaningful updates to the user

ACPMS connection bundle:
- Base Endpoint URL: https://api.yourdomain.com/api/openclaw/v1
- OpenAPI (Swagger): https://api.yourdomain.com/api/openclaw/openapi.json
- Guide Endpoint: https://api.yourdomain.com/api/openclaw/guide-for-openclaw
- Global Event SSE: https://api.yourdomain.com/api/openclaw/v1/events/stream
- API Key (Bearer): oc_live_5x8a9b2c3d4e5f6g7h8i9j0k
- Webhook Secret: wh_sec_a1b2c3d4e5f6g7h8i9j0k1l2 (optional)

Your required first actions:
1. Store the API Key as the Bearer credential for ACPMS.
2. Call the Guide Endpoint first with `GET` for basic bootstrap and treat its response as the authoritative runtime guide.
3. Load the OpenAPI document.
4. Open and maintain the Global Event SSE connection.
5. Use only ACPMS OpenClaw routes:
   - /api/openclaw/v1/*
   - /api/openclaw/ws/*
6. Follow the ACPMS operating rules returned by the Guide Endpoint.

Bootstrap example (curl):
```bash
curl -sS \
  -X GET \
  -H "Authorization: Bearer oc_live_5x8a9b2c3d4e5f6g7h8i9j0k" \
  "https://api.yourdomain.com/api/openclaw/guide-for-openclaw"
```

Human reporting rules:
- report important status, analyses, plans, started attempts, completed attempts, failed attempts, blocked work, and approval requests
- do not expose secrets, API keys, or webhook secrets in user-facing output
- distinguish clearly between:
  - what ACPMS currently says
  - what you recommend
  - what you already changed

Do not ask the user to manually map these ACPMS credentials unless strictly necessary.
Use the Guide Endpoint to bootstrap yourself automatically.
================================================================================
```

## 5. Prompt Design Rules

The installer-generated prompt should be:

*   short enough for a user to paste directly into OpenClaw
*   specific enough that OpenClaw knows its first next step
*   instance-specific, with real endpoint URLs and real credentials already embedded
*   subordinate to `/api/openclaw/guide-for-openclaw`, which remains the source of truth for detailed runtime policy

The installer prompt should **not** try to duplicate the full long-form rulebook.

Its job is only to:

1.  transfer the instance-specific connection bundle
2.  define the initial role and safety posture
3.  instruct OpenClaw to fetch the authoritative bootstrap guide

## 6. Relationship to `guide-for-openclaw`

The responsibilities are deliberately split:

### 6.1 Installer Prompt

The installer prompt is:

*   user-facing
*   copy-paste friendly
*   short
*   instance-specific
*   designed to be handed directly to OpenClaw

### 6.2 `guide-for-openclaw`

The bootstrap API response is:

*   machine-readable
*   detailed
*   authoritative
*   allowed to be much longer
*   the place where OpenClaw learns operating rules, reporting policy, runtime transport rules, and detailed action policy

## 7. Required OpenClaw Behavior After Receiving the Installer Prompt

After the user sends the installer prompt to OpenClaw, OpenClaw should:

1.  parse the embedded connection bundle
2.  store the ACPMS API key securely
3.  call `GET /api/openclaw/guide-for-openclaw` for basic bootstrap
4.  load the OpenAPI contract
5.  open the global event stream
6.  follow the returned operating rules for future user commands

At this point, the user should not need to re-explain:

*   what ACPMS is
*   which endpoints to call first
*   how OpenClaw should behave with ACPMS
*   what kinds of updates OpenClaw must report back

## 8. Security Rules

Because the installer prompt contains a privileged API key:

*   it must be shown only to the installing operator
*   it should be easy to copy, but not logged carelessly to shared channels
*   if saved to disk, the file should be created with restrictive permissions where practical
*   users should be warned that sending the prompt to an untrusted system is equivalent to sharing a Super Admin credential

## 9. Non-Goals

The installer prompt is not intended to:

*   replace the bootstrap API
*   replace OpenAPI discovery
*   encode the full ACPMS policy model inline
*   replace future structured OpenClaw settings UIs

It exists to make the first-time handoff from `install.sh` to OpenClaw fast and reliable.
