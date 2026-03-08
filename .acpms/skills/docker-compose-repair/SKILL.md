---
name: docker-compose-repair
description: Repair a broken Docker Compose-based preview or deploy runtime by fixing compose files, service wiring, ports, volumes, build context, or startup behavior until the runtime is actually reachable.
---

# Docker Compose Repair

## Objective
Make a compose-based runtime work for real, not just pass `docker compose config`
or image build validation.

## When This Applies
- `docker compose up -d --build` fails
- Compose starts but the expected service is not reachable
- Port mapping, volume, service name, or startup behavior is wrong
- Preview runtime verification fails for a compose-based setup

## Inputs
- `docker-compose.yml` or related compose files
- Dockerfile and build context
- Expected service URL/port
- Runtime logs and failing commands

## Workflow
1. Identify the actual compose file in use.
2. Validate syntax, then inspect service definitions, ports, env, volumes, and
   build context.
3. Start or restart the compose project.
4. Inspect container status and logs for the failing service.
5. Fix compose or Dockerfile issues until the target URL is reachable.
6. Re-verify with a real HTTP check.

## Decision Rules
| Situation | Action |
|---|---|
| Compose syntax is invalid | Fix the file before restarting |
| Services build but app does not listen | Fix startup command or runtime config |
| Port is occupied | Rebind host port and update preview contract |
| Compose is fundamentally wrong for the task | Replace or simplify it rather than layering more hacks |

## Output Contract
Emit:
- `compose_repair_status`
- `compose_repair_reason`
- `compose_changed_files`
- `compose_runtime_verification`

## Related Skills
- `preview-docker-runtime`
- `preview-runtime-diagnose`
- `build-artifact`

