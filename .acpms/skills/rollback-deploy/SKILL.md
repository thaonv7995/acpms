---
name: rollback-deploy
description: Restore the last known good deployment when the current release is unsafe, unhealthy, or clearly worse than the previous verified state.
---

# Rollback Deploy

## Objective
Return service to the last verified good deployment as quickly and safely as
possible when the current release should not remain active.

## When This Applies
- Health checks fail after deploy
- Critical endpoint regression is confirmed
- Routing or artifact changes created a severe outage
- A post-deploy smoke check recommends rollback

## Inputs
- Current failing deployment reference
- Last known good deployment reference
- Verification evidence for both current and previous state

## Workflow
1. Confirm rollback is warranted.
2. Identify the last known good deployment or route.
3. Perform the rollback for the active target.
4. Re-run post-rollback verification.
5. Report whether rollback fully recovered service or only partially improved it.

## Decision Rules
| Situation | Action |
|---|---|
| No verified previous deployment exists | Mark rollback blocked |
| Rollback succeeds and verification passes | Mark recovered |
| Rollback succeeds but verification is still degraded | Mark partial |
| Rollback command fails | Mark failed and keep incident open |

## Output Contract
Emit:
- `rollback_executed`
- `rollback_target`
- `rollback_reason`
- `rollback_status`: `recovered` | `partial` | `blocked` | `failed`
- `post_rollback_verification`

## Related Skills
- `post-deploy-smoke-and-healthcheck`
- `deploy-cancel-stop-cleanup`
- `cloudflare-incident-rollback`
