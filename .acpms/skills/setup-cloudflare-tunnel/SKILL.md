---
name: setup-cloudflare-tunnel
description: Prepare preview tunnel for Web/API and emit machine-parseable preview target fields. Agent must log user-friendly messages when tunnel fails (see Log for User).
---

# Setup Cloudflare Tunnel

## Objective
Publish a reachable public preview URL after the local preview runtime is already
working. This skill is responsible for turning a healthy local runtime into a
public preview contract, not for pretending a tunnel exists when it does not.

## When This Applies
- The local preview runtime is already reachable
- ACPMS wants a public preview URL in addition to the local `PREVIEW_TARGET`
- Cloudflare config is available in ACPMS-injected env vars

## Inputs
- Local runtime URL, usually `http://127.0.0.1:<port>`
- Cloudflare env:
  - `CLOUDFLARE_ACCOUNT_ID`
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - `CLOUDFLARE_BASE_DOMAIN`
- Existing `.acpms/preview-output.json` if preview metadata already exists

## Workflow
1. Confirm the local runtime is reachable with a real HTTP check.
2. Read Cloudflare config from env.
3. Create or resolve the tunnel route to the local runtime.
4. Verify the public route responds successfully.
5. Write or update `.acpms/preview-output.json`.
6. Emit `PREVIEW_TARGET` and `PREVIEW_URL`.

## Decision Rules
| Situation | Action |
|---|---|
| Local runtime is not reachable | Emit a failure reason and stop; do not create a tunnel. |
| Cloudflare is not configured | Emit the user-facing message and a root-cause failure reason. |
| Tunnel creation fails | Emit the user-facing message and `DEPLOYMENT_FAILURE_REASON`. |
| Public URL succeeds | Keep `PREVIEW_TARGET` local and set `PREVIEW_URL` to the public address. |

## Log for User
Use these messages when applicable:
- `Cloudflare is not configured. In System Settings (/settings), ensure Account ID, API Token, Zone ID, and Base Domain are all set.`
- `Cloudflare tunnel could not be created. In System Settings (/settings), ensure Account ID, API Token, Zone ID, and Base Domain are all set.`
- `Local service is not reachable. Check that the preview runtime is running.`

## Output Contract
- `PREVIEW_TARGET` must stay the local runtime URL
- `PREVIEW_URL` must be the public Cloudflare URL when tunnel creation succeeds
- If tunnel creation fails, emit `DEPLOYMENT_FAILURE_REASON: <root cause>`

## Guardrails
- Never output placeholder or fake URLs.
- Never output `PREVIEW_TARGET` if the local runtime is not reachable.
- Keep values machine-parseable and free of markdown wrappers.

## Related Skills
- `preview-docker-runtime`
- `deploy-precheck-cloudflare`
- `create-cloudflare-preview-tunnel`
- `cloudflare-dns-route`
