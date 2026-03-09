---
name: deploy-ssh-remote
description: Build locally, transfer artifacts to a configured SSH target, execute the remote deploy step, and verify the remote runtime before declaring success.
---

# Deploy SSH Remote

## Objective
Deploy to a remote server over SSH using ACPMS-provided deploy context, while
keeping the flow reproducible and reporting a real verified outcome.

## When This Applies
- The project deploy target is a remote server
- `.acpms/deploy/` exists with SSH deploy context
- Deployment should happen directly over SSH rather than through a platform API

## Inputs
- `.acpms/deploy/config.json`
- `.acpms/deploy/ssh_key`
- Built artifact or deployable repository state
- Remote deploy path and server identity

## Workflow
1. Confirm `.acpms/deploy/` exists and is usable.
2. Build the artifact or prepare the deployable state.
3. Verify the artifact exists locally.
4. Connect to the remote server with the provided SSH key.
5. Transfer artifacts or repository content to the configured deploy path.
6. Run the remote deploy/start command when required.
7. Verify the deployed service remotely or via its public/local endpoint.
8. Report the exact target and verification outcome.

## Decision Rules
| Situation | Action |
|---|---|
| Deploy context missing | Stop as `blocked` |
| Build fails | Stop; do not deploy |
| SSH connection fails | Report `failed` with connection cause |
| Transfer succeeds but runtime verification fails | Report `verification_failed` |
| All steps succeed | Report `success` |

## Guardrails
- Never log the SSH private key
- Never claim deploy success before verifying the remote runtime
- Never skip the build/artifact existence check unless the task explicitly uses
  source-only remote deploy

## Output Contract
Emit:
- `build_status`
- `artifact_paths`
- `deployment_status`
- `deploy_target`
- `post_deploy_verification`

## Related Skills
- `build-artifact`
- `post-deploy-smoke-and-healthcheck`
- `deploy-cancel-stop-cleanup`
