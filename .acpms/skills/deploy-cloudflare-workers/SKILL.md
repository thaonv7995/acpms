---
name: deploy-cloudflare-workers
description: Deploy an API or edge runtime to Cloudflare Workers, verify the endpoint, and report stable deployment metadata only after it is reachable.
---

# Deploy Cloudflare Workers

## Objective
Deploy a Workers runtime safely and report the resulting public endpoint only
after the worker or route is actually reachable.

## When This Applies
- The project deploy target is Cloudflare Workers
- A deployable worker package or config already exists
- The runtime should expose an HTTP endpoint or route

## Inputs
- Worker source or built package
- Worker configuration and bindings
- Cloudflare account credentials
- Expected endpoint or health path

## Workflow
1. Validate the worker configuration and required bindings.
2. Run the Workers deploy command.
3. Capture the resulting endpoint, route, or deployment reference.
4. Verify the deployed endpoint with a real HTTP request.
5. Only then report deployment success.

## Decision Rules
| Situation | Action |
|---|---|
| Worker build/package is missing | Stop with `no_artifact` |
| Deploy command fails | Report `deploy_failed` |
| Deploy succeeds but route is unreachable | Report `verification_failed` |
| Deploy and endpoint verification succeed | Report `active` |

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
