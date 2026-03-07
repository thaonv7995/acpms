---
name: deploy-precheck-cloudflare
description: Gate Web/API auto-deploy by checking Cloudflare readiness and applying skip policy when missing. Agent must log user-friendly message when Cloudflare is not configured (see Log for User).
---

# Deploy Precheck Cloudflare

## Objective
Enforce safe deploy gating for Web/API auto-deploy so attempts can still complete when Cloudflare is not configured. When Cloudflare is missing, **agent must output a message** for the user in the attempt log.

## Inputs
- `auto_deploy` flag resolved from task/project settings.
- Project type.
- Cloudflare configuration availability from env vars injected by ACPMS:
  - `CLOUDFLARE_ACCOUNT_ID`
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - `CLOUDFLARE_BASE_DOMAIN`

## Decision Table
| Condition | Result |
|---|---|
| `auto_deploy=false` | `DEPLOY_PRECHECK=not_required` |
| project type not `web` or `api` | `DEPLOY_PRECHECK=not_required` |
| `preview_enabled/auto_deploy` and all 4 Cloudflare env vars present | `DEPLOY_PRECHECK=ready` |
| `preview_enabled/auto_deploy` and any Cloudflare env var missing | `DEPLOY_PRECHECK=skipped_cloudflare_not_configured` |

## Log for User
**Agent must output this message** when Cloudflare is not configured—it appears in the attempt log (chat session). Do not use technical wording.

| Condition | Message to output |
|-----------|-------------------|
| Cloudflare missing (auto_deploy=true) | Cloudflare is not configured. Configure in System Settings (/settings) to enable preview. Task completed successfully. |

## Required Behavior When Cloudflare Missing
1. Do not execute deploy/tunnel steps.
2. **Output the Log for User message** so the user sees it in the attempt log.
3. Emit `DEPLOY_PRECHECK=skipped_cloudflare_not_configured`.
4. Continue completion flow (attempt succeeds).

## Output Contract
Include:
- `DEPLOY_PRECHECK`: `ready` | `not_required` | `skipped_cloudflare_not_configured`.
- `DEPLOY_PRECHECK_REASON`: one-line reason.
