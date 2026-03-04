---
name: cloudflare-tunnel-setup-guide
description: Guide for Cloudflare tunnel preview. Use when agent needs to output PREVIEW_TARGET or when Cloudflare tunnel/preview is involved. Agent must verify and report all 4 required System Settings fields (Account ID, API Token, Zone ID, Base Domain).
---

# Cloudflare Tunnel Setup Guide

## Constraint (Required)
When this skill is active (auto_deploy enabled), you **MUST** output PREVIEW_TARGET before finishing. Without it, the preview tunnel will be skipped.
- Start the preview runtime first (e.g. `docker compose up -d`, `npm run dev`).
- Then emit `PREVIEW_TARGET: http://127.0.0.1:<port>` or create `.acpms/preview-output.json`.

## Objective
Ensure preview tunnel works by guiding the agent to output correct fields and report when System Settings are incomplete. The **backend** creates the tunnel after the agent outputs `PREVIEW_TARGET`; if backend fails, the agent must help the user understand what to fix.

## Required System Settings (4 fields)
For tunnel creation to succeed, **all** of these must be set in System Settings (/settings):

| Field | Purpose |
|-------|---------|
| Cloudflare Account ID | Cloudflare account identifier |
| Cloudflare API Token | Authentication for Cloudflare API |
| Cloudflare Zone ID | DNS zone for custom subdomain (e.g. `task-xxx.yourdomain.com`) |
| Cloudflare Base Domain | Base domain for preview URLs |

If any is missing, the backend will fail with "Cloudflare tunnel could not be configured."

## Agent Workflow

### 1. When preview is needed (Web/API, auto_deploy)

**Option A — File contract (recommended):** Ghi `.acpms/preview-output.json`:
```json
{"preview_target": "http://127.0.0.1:3000", "preview_url": "https://..."}
```
- `mkdir -p .acpms && echo '{"preview_target":"http://127.0.0.1:3000"}' > .acpms/preview-output.json`

**Option B — Log output (fallback):** Output `PREVIEW_TARGET: http://127.0.0.1:<port>` (local runtime URL).
- Optionally output `PREVIEW_URL: https://...` when tunnel is created by backend.

### 2. When Cloudflare config might be incomplete
If the agent cannot verify config (e.g. no API to read settings), output this **before** or in the final report:

```
For preview tunnels, ensure System Settings (/settings) has all 4 fields:
- Cloudflare Account ID
- Cloudflare API Token  
- Cloudflare Zone ID
- Cloudflare Base Domain
```

### 3. When backend reports tunnel failure
If the attempt log shows "Cloudflare tunnel could not be configured", the agent should include in the final report:

```
Cloudflare tunnel could not be configured. In System Settings (/settings), ensure all 4 fields are set: Account ID, API Token, Zone ID, and Base Domain.
```

## Log for User
**Agent must output these messages** when applicable:

| Condition | Message to output |
|-----------|-------------------|
| Config missing or tunnel failed | Cloudflare tunnel could not be configured. In System Settings (/settings), ensure Cloudflare Account ID, API Token, Zone ID, and Base Domain are all set. **MUST** output `DEPLOYMENT_FAILURE_REASON: <explanation>`. |
| Local runtime not reachable | Local service is not reachable. Check that the dev server is running. **MUST** output `DEPLOYMENT_FAILURE_REASON: <explanation>`. |
| Cannot provide PREVIEW_TARGET | **MUST** output `DEPLOYMENT_FAILURE_REASON: <root cause>` (e.g. app failed to start, port conflict, docker compose error). User needs to know why. |

## Output Contract
- `PREVIEW_TARGET: http://127.0.0.1:<port>` (required when preview needed)
- `PREVIEW_URL: https://...` (optional, when available)
- User message when config/tunnel fails

## Coordination
- Works with `cloudflare-config-validate`, `setup-cloudflare-tunnel`, `create-cloudflare-preview-tunnel`.
- Zone ID and Base Domain are required for custom DNS; without them the backend may fail.
