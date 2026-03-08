---
name: init-microservice-scaffold
description: Create a microservice baseline with service entrypoint, health endpoints, containerization, and observability-ready structure.
---

# Init Microservice Scaffold

## Objective
Bootstrap a microservice that is container-ready, health-checkable, and
structured for future production deployment without adding unnecessary
complexity on day one.

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

## Workflow
1. Choose the runtime stack that matches explicit requirements.
2. Create the service entrypoint and configuration handling.
3. Add health endpoints and structured logging baseline.
4. Add containerization files and local run defaults.
5. If the service depends on supporting infrastructure, add
   `docker-compose.yml` that starts the service with those dependencies instead
   of leaving the runtime ambiguous.
6. Stub metrics or observability entrypoints when appropriate.
7. Leave the service runnable and health-checkable.

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
| Database need is unclear | Leave config hooks, avoid schema overcommit |
| Supporting services are required for the baseline runtime | Include `docker-compose.yml` from init, not as a later deploy-only patch. |
| Observability stack is unspecified | Add lightweight health/logging baseline first |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `created_files`
- `container_strategy`
- `verification_commands`

## Related Skills
- `init-project-bootstrap`
- `verify-test-build`
- `build-artifact`
