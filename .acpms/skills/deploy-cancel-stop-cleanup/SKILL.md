---
name: deploy-cancel-stop-cleanup
description: Cancel or stop a deployment/preview safely, clean up only the resources that belong to the current flow, and report exactly what was stopped or left running.
---

# Deploy Cancel Stop Cleanup

## Objective
Stop an in-flight or already-running deployment or preview safely, without
accidentally deleting unrelated runtime, data, or user-managed infrastructure.

## When This Applies
- User asks to stop or cancel a deploy
- User asks to stop preview runtime or tear down preview resources
- An auto-fix or recovery flow needs to clean up stale deployment state before
  retrying
- A preview container or compose project should be stopped and removed

## Inputs
- Current runtime control metadata, when available:
  - compose project name
  - container name
  - local port
  - process identifier
- Deployment context under `.acpms/deploy/`, when using SSH deploy flows
- User intent:
  - cancel only
  - stop runtime
  - remove runtime resources
  - aggressive cleanup

## Workflow
1. Determine whether the request is:
   - cancel orchestration only,
   - stop running runtime,
   - or stop and remove runtime resources.
2. Identify the narrowest owned resource set:
   - ACPMS-managed compose project
   - ACPMS-managed container
   - preview process by known port or PID
   - remote runtime via SSH deploy context
3. Stop the runtime cleanly using the owning tool first.
4. Remove containers or transient resources only when the task actually asks for
   cleanup.
5. Keep persistent data unless the task explicitly authorizes destructive
   cleanup.
6. Report exactly what was stopped, removed, skipped, or left intact.

## Stop Order
Prefer this order:

1. cancel orchestration run if still active
2. `docker compose down` for owned compose projects
3. `docker stop` for owned containers
4. process kill by known PID or local port
5. remote stop via SSH context

## Decision Rules
| Situation | Action |
|---|---|
| No owned runtime or deploy context can be identified | Report blocked; do not guess |
| User asked to cancel only | Cancel orchestration and keep runtime intact |
| Runtime belongs to current compose project | Use `docker compose down` |
| Runtime is a single owned container | Stop container directly |
| User did not request destructive cleanup | Do not remove volumes or unrelated images |

## Guardrails
- Never remove data volumes without explicit authorization
- Never run broad destructive commands such as `docker system prune -a` unless
  the user explicitly asks
- Never kill unrelated containers or processes just because they use Docker
- Never claim cleanup succeeded unless stop/remove commands actually returned
  success

## Output Contract
Emit:
- `cancel_status`: `success` | `skipped` | `failed`
- `cleanup_status`: `success` | `partial` | `failed` | `blocked`
- `containers_stopped`
- `processes_killed`
- `resources_removed`
- `cleanup_reason`

## Related Skills
- `preview-docker-runtime`
- `rollback-deploy`
- `retry-triage-and-recovery`
- `deploy-ssh-remote`
