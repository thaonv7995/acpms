---
name: create-cloudflare-preview-tunnel
description: Create a public Cloudflare preview URL for an already-running local preview runtime, then persist the correct ACPMS preview contract.
---

# Create Cloudflare Preview Tunnel

## Objective
Expose a working local preview runtime through a public Cloudflare URL and
persist the result without breaking the local preview contract.

This skill runs after the local runtime is already healthy. It should never be
used as a substitute for starting the runtime itself.

## When This Applies
- Local preview is already reachable
- ACPMS wants a public preview URL
- Cloudflare configuration is available
- The task is allowed to create or refresh preview routing

## Inputs
- Local runtime URL such as `http://127.0.0.1:<port>`
- ACPMS-injected env vars:
  - `CLOUDFLARE_ACCOUNT_ID`
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - `CLOUDFLARE_BASE_DOMAIN`
- Existing `.acpms/preview-output.json`, if the preview contract already exists

## Workflow
1. Confirm the local runtime is reachable with a real HTTP check.
2. Read and normalize Cloudflare env vars.
3. If Cloudflare config is incomplete, stop public preview work and keep local
   preview only.
4. Create the Cloudflare tunnel/public route for the local runtime.
5. Verify the resulting public preview URL.
6. Write `.acpms/preview-output.json` with:
   - local `preview_target`
   - public `preview_url`
   - runtime control metadata when available
7. Emit machine-parseable preview lines.

## Decision Rules
| Situation | Action |
|---|---|
| Local runtime not reachable | Stop; do not attempt Cloudflare |
| Cloudflare config missing | Keep local preview only and emit a clear message |
| Tunnel creation succeeds | Use public URL in `PREVIEW_URL` |
| Tunnel creation fails | Keep local preview only and emit `CLOUDFLARE_TUNNEL_ERROR` |

## Guardrails
- Never output a public URL as `PREVIEW_TARGET`
- Never claim public preview success before verifying the public URL
- Never hide a tunnel failure; emit the reason clearly
- Never fail the entire task only because public preview failed if local preview
  is still healthy and the wider task can continue

## Log for User
- `Cloudflare is not configured. Local preview is available, but public preview URL could not be created.`
- `Cloudflare tunnel could not be created. Local preview is still available.`
- `Local preview runtime is not reachable. Fix the runtime before creating a public preview URL.`

## Output Contract
Preferred file output:

```json
{
  "preview_target": "http://127.0.0.1:3000",
  "preview_url": "https://preview.example.com"
}
```

Required log lines:
- `PREVIEW_TARGET: http://127.0.0.1:<port>`
- `PREVIEW_URL: <public-or-local-url>`
- `CLOUDFLARE_TUNNEL_ERROR: <reason>` when applicable

## Related Skills
- `cloudflare-config-validate`
- `setup-cloudflare-tunnel`
- `cloudflare-dns-route`
- `preview-docker-runtime`
