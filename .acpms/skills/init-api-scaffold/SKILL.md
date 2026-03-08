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

## Workflow
1. Choose the safest baseline stack that matches explicit requirements.
2. Create the core project manifest and entrypoint.
3. Add configuration handling and `.env.example`.
4. Create at least:
   - health endpoint
   - versioned API root
   - structured error handling shape
5. Add Docker/dev defaults only when they support the baseline immediately.
6. Leave the project in a runnable, verifiable state.

## Required Baseline
- project manifest
- source entrypoint
- health endpoint
- API version prefix
- README
- `.gitignore`
- `.env.example`

## Decision Rules
| Situation | Action |
|---|---|
| Framework is explicitly requested | Follow it |
| Database need is unclear | Stub config, do not invent a full schema |
| Auth is not in scope yet | Leave auth-ready structure but do not over-implement |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `created_files`
- `verification_commands`
- `assumptions`

## Related Skills
- `init-project-bootstrap`
- `verify-test-build`
- `init-project-context-file`
