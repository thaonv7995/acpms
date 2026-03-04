---
name: update-deployment-metadata
description: Produce metadata-aligned deploy summary fields for preview and production outcomes.
---

# Update Deployment Metadata

## Objective
Ensure deploy outcomes are represented with clear, machine-readable metadata semantics.

## Context
In this system, backend persists task metadata patch before marking attempt success. Agent should emit consistent values so backend and reports stay aligned.

## Metadata Semantics
Use these keys and value patterns in report content when applicable:
- `deployment_status`: `active` | `missing_preview_target` | `skipped_cloudflare_not_configured`
- `deployment_error`: reason when deployment preview/status is not active
- `deployment_kind`: `agent_preview_url` | `preview_tunnel` | `artifact_downloads`
- `preview_target`: `http://127.0.0.1:<port>` when available
- `preview_url`: public preview URL when available
- `production_deployment_status`: `active` | `build_failed` | `deploy_failed` | `no_artifact` | `skipped_cloudflare_not_configured`
- `production_deployment_error`: reason when production deploy not active
- `production_deployment_url`, `production_deployment_type`, `production_deployment_id`: success fields

## Workflow
1. Determine deploy outcome class: success, failed, or skipped.
2. Map outcome to canonical statuses above.
3. Emit a compact metadata-aligned summary in final report.

## Output Contract
Provide `Metadata Patch Summary` as YAML-like lines for easy parsing, for example:
- `deployment_status: active`
- `deployment_kind: preview_tunnel`
- `production_deployment_status: deploy_failed`
- `production_deployment_error: Auto-deploy failed: ...`
