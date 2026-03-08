---
name: deploy-precheck-cloudflare
description: Gate Web/API auto-deploy by checking Cloudflare readiness and applying skip policy when missing. Agent must log user-friendly message when Cloudflare is not configured (see Log for User).
---

# Deploy Precheck Cloudflare

## Objective
Gate Cloudflare preview or auto-deploy safely so a task can still succeed when
Cloudflare is intentionally not configured. This skill decides whether the
Cloudflare path is ready, not whether the whole task should fail.

## When This Applies
- ACPMS has `preview_enabled` or `auto_deploy` semantics for a web or API project
- The flow needs to decide whether to attempt Cloudflare preview/public deploy
- A task should still complete cleanly when Cloudflare is unavailable

## Inputs
- `auto_deploy` and preview-related flags resolved from task or project settings
- Project type
- ACPMS-injected Cloudflare env:
  - `CLOUDFLARE_ACCOUNT_ID`
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - `CLOUDFLARE_BASE_DOMAIN`

## Workflow
1. Resolve whether the current task actually requires deploy or preview gating.
2. Check project type before touching Cloudflare-specific logic.
3. Validate whether all required Cloudflare env vars are present.
4. Emit one clear status:
   - `ready`
   - `not_required`
   - `skipped_cloudflare_not_configured`
5. If skipped, log the user-facing message and let the broader task continue.

## Decision Rules
| Situation | Action |
|---|---|
| Web/API project with preview/deploy enabled and all vars present | Mark `DEPLOY_PRECHECK=ready`. |
| Preview/deploy is not required for this task | Mark `DEPLOY_PRECHECK=not_required`. |
| Any required Cloudflare var is missing | Mark `skipped_cloudflare_not_configured`, emit the user-facing log message, and stop Cloudflare steps only. |

## Log for User
When Cloudflare is missing, emit:

`Cloudflare is not configured. Configure it in System Settings (/settings) to enable preview. Task completed successfully.`

## Output Contract
Emit:
- `DEPLOY_PRECHECK`
- `DEPLOY_PRECHECK_REASON`

## Related Skills
- `preview-docker-runtime`
- `setup-cloudflare-tunnel`
- `create-cloudflare-preview-tunnel`
- `update-deployment-metadata`
