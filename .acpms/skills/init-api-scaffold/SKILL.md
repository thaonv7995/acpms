---
name: init-api-scaffold
description: Create a minimal but production-shaped API service scaffold with a runnable entrypoint, health endpoint, env/config structure, and verification-ready project layout.
---

# Init API Scaffold

## Objective
Create a new API service baseline that is runnable, structured, and easy to
extend without overbuilding infrastructure before the first useful endpoint
exists.

## When This Applies
- Project type is API or backend service
- ACPMS is bootstrapping a new service from scratch

## Inputs
- Project name and brief
- Required language/framework, if specified
- Expected persistence or auth needs, if known
- Preview/deploy expectation, especially if ACPMS preview should run from Docker
- Supporting services such as database, cache, queue, broker, or worker

## Workflow
1. Choose the safest baseline stack that matches explicit requirements.
2. Create the core project manifest and entrypoint.
3. Add configuration handling and `.env.example`.
4. Create at least:
   - health endpoint
   - versioned API root
   - structured error handling shape
5. Add container runtime files whenever ACPMS preview/deploy or helper services
   require them:
   - `Dockerfile`
   - `docker-compose.yml` when orchestration or support services are needed
6. Add Docker/dev defaults only when they support the baseline immediately.
7. Leave the project in a runnable, verifiable state.

## Required Baseline
- project manifest
- source entrypoint
- health endpoint
- API version prefix
- README
- `.gitignore`
- `.env.example`
- `Dockerfile` when the service is expected to run in ACPMS Docker preview/deploy
- `docker-compose.yml` when helper services are part of the baseline runtime

## Decision Rules
| Situation | Action |
|---|---|
| Framework is explicitly requested | Follow it |
| Database need is unclear | Stub config, do not invent a full schema |
| Auth is not in scope yet | Leave auth-ready structure but do not over-implement |
| ACPMS preview/deploy expects Docker runtime | Include a working `Dockerfile` from init. |
| Service depends on DB/cache/queue/broker | Include `docker-compose.yml` wiring for the app and support services. |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `created_files`
- `container_strategy`
- `verification_commands`
- `assumptions`

## Related Skills
- `init-project-bootstrap`
- `verify-test-build`
- `init-project-context-file`
