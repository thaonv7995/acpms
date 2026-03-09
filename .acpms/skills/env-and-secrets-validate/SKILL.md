---
name: env-and-secrets-validate
description: Validate required env vars and secrets for the current task context before build, preview, deploy, or external integration steps continue.
---

# Env And Secrets Validate

## Objective
Fail early on missing or obviously invalid configuration so ACPMS does not waste
time on build, preview, or deploy steps that cannot succeed.

## When This Applies
- Build depends on environment variables
- Preview or deploy depends on external provider credentials
- CI, tunnel, or remote runtime setup needs secrets
- The task explicitly mentions env, secret, token, account, or credential setup

## Inputs
- Required env var names for the current flow
- Optional env vars that improve capability but do not block execution
- Task execution mode:
  - build
  - local preview
  - public preview
  - deploy
  - CI or integration

## Workflow
1. Determine the env/secret checklist for the active task.
2. Check only presence, basic shape, and non-empty values.
3. Separate blocking from optional configuration.
4. Stop dependent steps when blocking secrets are missing or clearly malformed.
5. Report variable names only, never values.

## Decision Rules
| Situation | Action |
|---|---|
| Required secret missing | Block dependent step |
| Optional variable missing | Continue with warning |
| Value is present but obviously invalid or masked | Treat as invalid config |
| All blocking vars are present | Mark ready |

## Guardrails
- Never print secret values
- Never log decrypted credentials
- Never continue a deploy/public-preview step when the required credentials are
  missing

## Output Contract
Emit:
- `env_check_status`: `ready` | `partial` | `blocked`
- `missing_required_env`
- `missing_optional_env`
- `blocked_steps`

## Related Skills
- `task-preflight-check`
- `cloudflare-config-validate`
- `deploy-precheck-cloudflare`
