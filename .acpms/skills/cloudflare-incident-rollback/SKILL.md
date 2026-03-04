---
name: cloudflare-incident-rollback
description: Roll back Cloudflare deployment/routing to last known good state during incidents.
---

# Cloudflare Incident Rollback

## Objective
Restore service quickly when Cloudflare deploy or routing changes break traffic.

## Inputs
- Last known good deployment/routing reference.
- Current failing deployment details.

## Workflow
1. Identify failing component (artifact deploy, worker route, DNS, tunnel).
2. Revert to last known good deployment/route.
3. Verify health from public entrypoint.
4. Record rollback action and impact window.

## Decision Rules
| Situation | Action |
|---|---|
| No known-good reference | Escalate and report rollback blocked. |
| Rollback succeeds but health still degraded | Keep incident open and report remaining failure domain. |

## Output Contract
Include:
- `cloudflare_rollback_executed`: `true` | `false`
- `cloudflare_rollback_target`
- `cloudflare_rollback_status`: `recovered` | `partial` | `blocked`
- `cloudflare_rollback_reason`
