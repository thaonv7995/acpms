---
name: post-deploy-smoke-and-healthcheck
description: Run post-deploy smoke and health checks to validate availability and critical path behavior.
---

# Post Deploy Smoke And Healthcheck

## Objective
Validate that deployment is reachable and core flows function before declaring success.

## Inputs
- Deployment URL or API endpoint.
- Critical endpoints/routes for the project.
- Expected healthy response criteria.

## Workflow
1. Verify DNS/URL reachability.
2. Run health endpoint check (`/health` or equivalent).
3. Execute 1-3 critical smoke checks.
4. Capture response code, latency, and basic payload assertions.
5. Classify outcome and recommend rollback if critical checks fail.

## Decision Rules
| Situation | Action |
|---|---|
| Health check fails hard | Mark deploy unstable and trigger rollback evaluation. |
| Non-critical smoke check fails | Mark degraded and report risk. |
| All critical checks pass | Mark deploy validated. |

## Output Contract
Include:
- `smoke_status`: `validated` | `degraded` | `failed`
- `healthcheck_url`
- `critical_checks`
- `rollback_recommended`: `true` | `false`
