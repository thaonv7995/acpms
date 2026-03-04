---
name: deploy-cloudflare-pages
description: Deploy Web artifacts to Cloudflare Pages and report production deployment results.
---

# Deploy Cloudflare Pages

## Objective
Deploy Web build output to Cloudflare Pages and provide reliable deployment evidence.

## Inputs
- Valid Web artifact output.
- Cloudflare auth/project binding for Pages.

## Workflow
1. Verify artifact path exists and matches Pages deployment expectations.
2. Run Pages deploy command from project workflow.
3. Capture deployment URL and deployment identifier.
4. Run smoke validation against deployed URL.

## Decision Rules
| Situation | Action |
|---|---|
| No artifact available | Mark as `no_artifact` and stop deploy. |
| Deploy command fails | Mark as `deploy_failed` and include root cause stage. |
| Deploy succeeds | Mark as active and record URL + deployment ID. |

## Output Contract
Include `Production Deploy` section:
- `production_deployment_status`: `active` | `deploy_failed` | `no_artifact` | `build_failed`.
- `production_deployment_url`: URL when success.
- `production_deployment_type`: provider/type label when available.
- `production_deployment_id`: ID/reference when available.
