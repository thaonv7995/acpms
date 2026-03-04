---
name: rollback-deploy
description: Roll back to a known-good deployment when current release is unsafe or non-functional.
---

# Rollback Deploy

## Objective
Restore service stability quickly when deployment introduces critical regressions.

## Trigger Conditions
- Health checks fail after deploy.
- Critical endpoint regression detected.
- Invalid routing/configuration causes outage.

## Workflow
1. Identify last known good deployment reference.
2. Execute rollback for the active target.
3. Verify service health post-rollback.
4. Record rollback reason, target reference, and verification evidence.

## Decision Rules
| Situation | Action |
|---|---|
| No verified previous deployment exists | Escalate and mark rollback blocked. |
| Partial recovery only | Report degraded state and next mitigation steps. |

## Output Contract
Include `Rollback Summary`:
- `rollback_executed`: `true` or `false`.
- `rollback_target`: deployment ref/ID.
- `rollback_reason`: concise root cause.
- `post_rollback_verification`: checks and result.
