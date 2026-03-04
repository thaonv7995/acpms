---
name: deploy-cloudflare-workers
description: Deploy API runtime to Cloudflare Workers and report endpoint/deployment metadata.
---

# Deploy Cloudflare Workers

## Objective
Deploy API runtime to Workers with clear endpoint and deployment status reporting.

## Inputs
- Build artifact or deployable worker package.
- Worker configuration/routes/environment bindings.

## Workflow
1. Validate worker configuration and required env bindings.
2. Run Worker deploy command.
3. Capture endpoint URL, deployment ID/version, and route mapping.
4. Run smoke/health check against deployed endpoint.

## Decision Rules
| Situation | Action |
|---|---|
| Build artifact missing | Mark `no_artifact`. |
| Build failed before deploy | Mark `build_failed`. |
| Worker deploy fails | Mark `deploy_failed` and include failing phase. |
| Success | Mark active and include endpoint + deployment ref. |

## Output Contract
Include `Production Deploy` section:
- `production_deployment_status`: `active` | `deploy_failed` | `no_artifact` | `build_failed`.
- `production_deployment_url`: endpoint URL when success.
- `production_deployment_type`: provider/type label when available.
- `production_deployment_id`: deployment ref when available.
