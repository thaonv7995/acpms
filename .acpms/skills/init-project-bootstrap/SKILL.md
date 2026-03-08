---
name: init-project-bootstrap
description: Execute init-task bootstrap workflow for new projects with safe defaults and reproducible setup.
---

# Init Project Bootstrap

## Objective
Create a runnable baseline project from init requirements without over-scaffolding,
unsafe defaults, or optional tooling that slows the first usable state.

## When This Applies
- ACPMS is handling a from-scratch project initialization
- A project brief exists but the repo still needs its first runnable baseline
- Type-specific scaffold skills need a shared bootstrap policy first

## Inputs
- Init task metadata and selected stack
- Existing repository state
- Required project settings such as visibility, naming, and base structure

## Workflow
1. Parse the init scope and any explicit stack requirements.
2. Choose the simplest bootstrap path that satisfies those requirements.
3. Generate the minimum runnable skeleton.
4. Install dependencies and run baseline validation.
5. Produce a concise bootstrap summary for downstream init steps.

## Decision Rules
| Situation | Action |
|---|---|
| Stack selection is incomplete | Apply the safest default and report the assumption. |
| Bootstrap command fails | Stop, capture root cause, and provide recovery guidance. |
| Optional integrations are unavailable | Continue with the core scaffold and mark optional setup pending. |

## Output Contract
Emit:
- `init_status`
- `stack_selected`
- `bootstrap_commands`
- `bootstrap_validation`
- `bootstrap_assumptions`

## Related Skills
- `task-preflight-check`
- `init-web-scaffold`
- `init-source-repository`
- `verify-test-build`
