---
name: update-deployment-metadata
description: Produce metadata-aligned deploy summary fields for preview and production outcomes.
---

# Update Deployment Metadata

## Objective
Represent deploy outcomes with clear, machine-readable metadata semantics so the
backend, UI, and final report stay aligned.

## When This Applies
- Preview or production deploy state changed during the attempt
- ACPMS needs metadata-aligned summary fields for UI, logs, or downstream automation
- A deploy was skipped, failed, or only partially succeeded and the reason must stay machine-readable

## Inputs
- Final preview outcome (`PREVIEW_TARGET`, `PREVIEW_URL`, runtime state)
- Production deploy outcome when applicable
- Skip or failure reason if a deploy stage did not succeed
- Current task type and whether deploy was actually in scope

## Workflow
1. Determine the deploy outcome class: success, failed, skipped, or stale.
2. Map the outcome to ACPMS canonical metadata values.
3. Emit a compact metadata-aligned summary in the final report or metadata block.

## Decision Rules
| Situation | Action |
|---|---|
| Local preview exists but no public URL exists | Keep `preview_target` local and set `preview_url` to the same local URL unless a public URL was really created. |
| Deploy was intentionally skipped by policy | Mark it as skipped and include the explicit reason. |
| Preview/runtime was stale or cleaned up | Do not mark deployment active; preserve the stop or failure reason. |

## Output Contract
Provide `Metadata Patch Summary` as YAML-like lines, for example:
- `deployment_status: active`
- `deployment_kind: preview_tunnel`
- `production_deployment_status: deploy_failed`
- `production_deployment_error: Auto-deploy failed: ...`

## Related Skills
- `deploy-precheck-cloudflare`
- `preview-docker-runtime`
- `setup-cloudflare-tunnel`
- `final-report`
