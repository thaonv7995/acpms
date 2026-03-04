---
name: init-project-bootstrap
description: Execute init-task bootstrap workflow for new projects with safe defaults and reproducible setup.
---

# Init Project Bootstrap

## Objective
Create a runnable baseline project from init requirements without over-scaffolding or unsafe defaults.

## Inputs
- Init task metadata and selected stack.
- Repository state for initial commit branch.
- Required project settings (visibility, naming, base structure).

## Workflow
1. Parse init scope and selected stack/framework.
2. Generate minimal project skeleton needed for first runnable state.
3. Install dependencies and run baseline validation.
4. Create starter docs/config only required for build/run.
5. Produce a concise bootstrap summary.

## Decision Rules
| Situation | Action |
|---|---|
| Stack selection is incomplete | Apply safest default and report assumption. |
| Bootstrap command fails | Stop, capture root cause, provide recovery steps. |
| Optional integrations unavailable | Continue with core app scaffold and mark optional setup pending. |

## Guardrails
- Keep initialization deterministic and reproducible.
- Do not add optional components unless required by init scope.

## Output Contract
Include:
- `init_status`: `success` | `failed` | `partial`
- `stack_selected`
- `bootstrap_commands`
- `bootstrap_validation`
- `bootstrap_assumptions`
