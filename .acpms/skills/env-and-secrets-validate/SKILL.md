---
name: env-and-secrets-validate
description: Validate required environment variables and secrets before build, deploy, or integration steps.
---

# Env And Secrets Validate

## Objective
Fail early on missing runtime/deploy prerequisites and avoid false-negative execution outcomes.

## Inputs
- Required env var list from project config/scripts.
- Secret-backed settings required by external providers.
- Task execution mode (build-only, deploy, tunnel, CI).

## Workflow
1. Build required-variable checklist per task context.
2. Check presence/non-empty values without printing secret content.
3. Classify missing variables by severity (blocking vs optional).
4. Block unsafe steps when blocking variables are absent.

## Decision Rules
| Situation | Action |
|---|---|
| Required secret missing | Stop dependent step and mark blocked. |
| Optional variable missing | Continue with warning and impact note. |
| Secret present but invalid format | Mark config invalid and request correction. |

## Guardrails
- Never log plain secret values.
- Report variable names only, never values.

## Output Contract
Include:
- `env_check_status`: `ready` | `partial` | `blocked`
- `missing_required_env`
- `missing_optional_env`
- `blocked_steps`
