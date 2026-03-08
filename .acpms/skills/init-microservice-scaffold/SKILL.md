---
name: init-microservice-scaffold
description: Create a microservice baseline with service entrypoint, health endpoints, containerization, and observability-ready structure.
---

# Init Microservice Scaffold

## Objective
Bootstrap a microservice that is container-ready, health-checkable, and
structured for future production deployment without adding unnecessary
complexity on day one.

Do not assume every microservice lives in a monorepo. Infer whether this is a
single-service repo or a multi-service monorepo from the brief or existing
repository layout.

## When This Applies
- Project type is microservice
- A service baseline needs containerization and operational structure from the
  start

## Inputs
- Project brief
- Language/runtime constraints
- Communication style, if known:
  - REST
  - gRPC
  - async jobs
- Repo-shape clues:
  - standalone service repo
  - multi-service monorepo
  - imported workspace with existing layout

## Workflow
1. Decide repo shape from the brief or existing layout:
   - standalone service repo
   - service inside a monorepo
2. Choose the runtime stack that matches explicit requirements.
3. Create the service entrypoint and configuration handling.
4. Add health endpoints and structured logging baseline.
5. If the repo shape is monorepo, place the service in the correct scoped
   directory and avoid treating the whole repo as one service.
6. Add containerization files and local run defaults.
7. If the service depends on supporting infrastructure, add
   `docker-compose.yml` that starts the service with those dependencies instead
   of leaving the runtime ambiguous.
8. Stub metrics or observability entrypoints when appropriate.
9. Leave the service runnable and health-checkable.

## Required Baseline
- service entrypoint
- `.env.example`
- health endpoint
- Dockerfile
- README

## Decision Rules
| Situation | Action |
|---|---|
| Communication pattern is unclear | Start with a health endpoint and simplest external interface |
| Brief implies one bounded service only | Scaffold a single-service repo layout |
| Brief implies multiple services in one repo | Use a monorepo-friendly layout and keep this scaffold scoped to one service |
| Existing repo already contains multiple services/packages | Preserve the monorepo shape and add or normalize only the targeted service |
| Database need is unclear | Leave config hooks, avoid schema overcommit |
| Supporting services are required for the baseline runtime | Include `docker-compose.yml` from init, not as a later deploy-only patch. |
| Observability stack is unspecified | Add lightweight health/logging baseline first |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `repo_shape_selected`
- `created_files`
- `container_strategy`
- `verification_commands`

## Related Skills
- `init-project-bootstrap`
- `monorepo-service-selector`
- `verify-test-build`
- `build-artifact`
