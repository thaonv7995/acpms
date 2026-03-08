---
name: post-deploy-smoke-and-healthcheck
description: Validate a deployment with a small set of real health and smoke checks before it is reported as good.
---

# Post Deploy Smoke And Healthcheck

## Objective
Confirm that a deployment is actually reachable and that the most important
paths still work before ACPMS reports the deploy as healthy.

## When This Applies
- A preview or production deployment just completed
- A route was changed and needs verification
- A rollback finished and needs confirmation

## Inputs
- Deployment URL or endpoint
- Health path, if available
- One to three critical smoke checks

## Workflow
1. Check basic reachability for the deployment URL.
2. Run the health endpoint when one exists.
3. Run one to three critical smoke checks.
4. Record status code, key assertion, and any obvious regression.
5. Decide whether deploy is validated, degraded, or failed.

## Decision Rules
| Situation | Action |
|---|---|
| Base URL is unreachable | Mark `failed` |
| Health endpoint fails | Mark `failed` and consider rollback |
| Non-critical smoke check fails | Mark `degraded` |
| All critical checks pass | Mark `validated` |

## Output Contract
Emit:
- `smoke_status`: `validated` | `degraded` | `failed`
- `healthcheck_url`
- `critical_checks`
- `rollback_recommended`: `true` | `false`

## Related Skills
- `deploy-cloudflare-pages`
- `deploy-cloudflare-workers`
- `deploy-ssh-remote`
- `rollback-deploy`
