---
name: deploy-cloudflare-pages
description: Deploy built web artifacts to Cloudflare Pages, verify the deployed URL, and report stable deployment metadata.
---

# Deploy Cloudflare Pages

## Objective
Publish a web artifact to Cloudflare Pages and return trustworthy deployment
metadata only after the deployed URL actually works.

## When This Applies
- The project is a web/static app
- Build artifacts already exist
- The target deploy platform is Cloudflare Pages

## Inputs
- Built artifact directory, usually `dist/` or equivalent
- Cloudflare account credentials
- Pages project binding or deploy target name
- Expected health or smoke route

## Workflow
1. Confirm the build artifact exists and looks deployable.
2. Confirm Cloudflare credentials or Pages auth context is present.
3. Run the Pages deploy command for the artifact.
4. Capture deployment URL and deployment reference.
5. Verify the deployed URL with a real HTTP check.
6. Return deployment metadata only after verification succeeds.

## Decision Rules
| Situation | Action |
|---|---|
| Artifact directory missing | Stop with `no_artifact` |
| Deploy command fails | Stop with `deploy_failed` and report the failing stage |
| Deploy succeeds but verification fails | Mark deploy as failed or degraded; do not claim active success |
| Deploy and verification both succeed | Report active deployment |

## Output Contract
Emit:
- `production_deployment_status`: `active` | `deploy_failed` | `no_artifact` | `build_failed` | `verification_failed`
- `production_deployment_url`
- `production_deployment_type`
- `production_deployment_id`

## Related Skills
- `build-artifact`
- `post-deploy-smoke-and-healthcheck`
- `rollback-deploy`
