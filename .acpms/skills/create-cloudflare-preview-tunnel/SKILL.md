---
name: create-cloudflare-preview-tunnel
description: Create Cloudflare tunnel for preview. Use when agent needs to expose local runtime for preview. On config missing or error, agent must log a user-friendly message (see Log for User).
---

# Create Cloudflare Preview Tunnel

## Objective
Create Cloudflare tunnel to expose local runtime for preview. Read config from ACPMS-injected env vars (`CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ZONE_ID`, `CLOUDFLARE_BASE_DOMAIN`). When config is missing or tunnel creation fails, **agent must output a message** that appears in the attempt log for the user.

## Inputs
- Local runtime port (e.g. from dev-server)
- Env vars injected by ACPMS:
  - `CLOUDFLARE_ACCOUNT_ID`
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - `CLOUDFLARE_BASE_DOMAIN`

## Workflow
1. Read Cloudflare config from the env vars above.
2. If config missing or invalid → output message for user (see Log for User).
3. Create tunnel route to `http://127.0.0.1:<port>`.
4. Validate route responds.
5. Emit preview fields.

## Required Output (Success)

### Option A — File contract (recommended)
Ghi `.acpms/preview-output.json`: `{"preview_target": "http://127.0.0.1:<port>", "preview_url": "https://..."}`

### Option B — Log output (fallback)
- `PREVIEW_TARGET: http://127.0.0.1:<port>`
- `PREVIEW_URL: https://...` (when tunnel created)

## Log for User
**Agent must output these messages** when they occur—they appear in the attempt log (chat session). Do not rely on backend to log; the agent reports to the user.

| Condition | Message to output |
|-----------|-------------------|
| Config missing (account_id or api_token) | Cloudflare is not configured. Configure in System Settings (/settings) to enable preview. Task completed successfully. |
| Token invalid / tunnel creation failed | Cloudflare tunnel could not be created. Check System Settings (/settings). Task completed successfully. |
| Local runtime not reachable | Local service is not reachable. Check that the dev server is running. |

Also emit machine-parseable lines when applicable:
- `CLOUDFLARE_CONFIG_NEEDED: true` (when config missing)
- `CLOUDFLARE_TUNNEL_ERROR: <short reason>` (when tunnel failed)

## Decision Rules
| Situation | Action |
|-----------|--------|
| Config present, tunnel OK | Emit PREVIEW_TARGET + PREVIEW_URL |
| Config missing | Output Log for User message + CLOUDFLARE_CONFIG_NEEDED |
| Token invalid / tunnel failed | Output Log for User message + CLOUDFLARE_TUNNEL_ERROR |
| Local runtime not reachable | Output Log for User message; do not blame Cloudflare |

## Guardrails
- Never hardcode credentials; read only from the ACPMS-injected env vars.
- On error: agent must output the Log for User message—do not assume backend will log.
- Tunnel/deployment errors must not fail the attempt (handled by backend).
- When Cloudflare config is present, do not silently stop at local preview only. Try to create the public URL first, then fall back only if tunnel creation really fails.
