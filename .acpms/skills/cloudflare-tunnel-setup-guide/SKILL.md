---
name: cloudflare-tunnel-setup-guide
description: Explain and enforce the ACPMS contract for public preview tunnels: local runtime first, Cloudflare public URL second, and machine-parseable preview fields at the end.
---

# Cloudflare Tunnel Setup Guide

## Objective
Guide the agent through the correct ACPMS public preview contract so it does not
confuse local runtime setup, preview verification, and Cloudflare public URL
creation.

This skill is instructional and coordinating. Use it to keep the preview flow
correct. It does not replace the execution skills that start the local runtime
or create the actual tunnel.

## When This Applies
- A task has preview delivery enabled
- The task needs a public preview URL
- The agent is about to set up or report Cloudflare preview information
- The agent needs a reminder of what `PREVIEW_TARGET` and `PREVIEW_URL` mean

## Inputs
- Local preview status
- ACPMS-injected Cloudflare env vars, when available:
  - `CLOUDFLARE_ACCOUNT_ID`
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - `CLOUDFLARE_BASE_DOMAIN`
- Existing `.acpms/preview-output.json`, if present

## Core Contract
- `PREVIEW_TARGET` must always be the local runtime URL
- `PREVIEW_URL` is the URL the UI should open
- If no public URL exists, `PREVIEW_URL` may equal the local `PREVIEW_TARGET`
- If a Cloudflare public URL exists, keep:
  - `PREVIEW_TARGET = http://127.0.0.1:<port>`
  - `PREVIEW_URL = https://...`

## Workflow
1. Bring up the local preview runtime first.
2. Verify the local runtime with a real HTTP request.
3. Set or emit `PREVIEW_TARGET` only after the local runtime responds.
4. If Cloudflare config is available, attempt to create a public URL.
5. If the public URL succeeds, set `PREVIEW_URL` to that public address.
6. If Cloudflare is unavailable or tunnel creation fails, keep `PREVIEW_URL`
   equal to the local preview URL.
7. Write `.acpms/preview-output.json` and emit machine-parseable preview lines.

## Required File Contract
Preferred output:

```json
{
  "preview_target": "http://127.0.0.1:3000",
  "preview_url": "https://preview.example.com",
  "runtime_control": {
    "controllable": true,
    "runtime_type": "docker_compose_project",
    "compose_project_name": "example-preview"
  }
}
```

If no public URL exists, this is still valid:

```json
{
  "preview_target": "http://127.0.0.1:3000",
  "preview_url": "http://127.0.0.1:3000"
}
```

## Decision Rules
| Situation | Action |
|---|---|
| Local runtime is not reachable | Do not emit preview fields; fix runtime first |
| Local runtime works, Cloudflare not configured | Use local URL for both `PREVIEW_TARGET` and `PREVIEW_URL` |
| Local runtime works, Cloudflare public URL succeeds | Keep local target, set public preview URL |
| Cloudflare tunnel fails | Keep local preview contract and emit a clear tunnel error |

## Guardrails
- Never output a public URL as `PREVIEW_TARGET`
- Never emit `PREVIEW_TARGET` before verifying the local runtime
- Never leave `preview_url` empty when a valid local preview exists
- Never claim Cloudflare succeeded unless the public URL was actually created

## Log for User
- `Cloudflare is not configured. Local preview is available, but public preview URL could not be created.`
- `Cloudflare tunnel could not be created. Local preview is still available.`
- `Local preview runtime is not reachable yet.`

## Output Contract
Emit:
- `PREVIEW_TARGET: http://127.0.0.1:<port>`
- `PREVIEW_URL: <public-or-local-url>`
- `CLOUDFLARE_TUNNEL_ERROR: <reason>` when Cloudflare was attempted but failed

## Related Skills
- `preview-docker-runtime`
- `cloudflare-config-validate`
- `create-cloudflare-preview-tunnel`
- `setup-cloudflare-tunnel`
- `update-deployment-metadata`
