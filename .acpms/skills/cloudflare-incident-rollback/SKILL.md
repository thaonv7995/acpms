---
name: cloudflare-incident-rollback
description: Roll back Cloudflare preview or deployment routing to the last known good state when a Cloudflare change breaks traffic, preview access, or public DNS.
---

# Cloudflare Incident Rollback

## Objective
Recover service or preview access quickly when a Cloudflare-side change causes
an outage, bad routing, broken public preview, or traffic pointed at the wrong
target.

This skill is Cloudflare-specific. Use it when the failure domain is in tunnel,
DNS, public hostname, or Cloudflare deployment routing, not when the app itself
is simply broken locally.

## When This Applies
- A new Cloudflare tunnel or DNS route broke public preview access
- A Cloudflare deploy changed routing and traffic now points to a bad target
- Public hostname resolves, but to the wrong endpoint
- A rollback to the last known good Cloudflare-facing state is safer than
  continuing forward fixes during the incident window

Do **not** use this skill when:
- there is no verified last-known-good state
- the root problem is only local app code and Cloudflare routing is still
  correct
- the requested action is routine cleanup rather than incident rollback

## Inputs
- Current failing component:
  - tunnel
  - DNS route
  - public preview URL
  - Cloudflare deployment route
- Last known good reference, such as:
  - previous tunnel/public URL
  - previous DNS target
  - previous deployment/route ref
- Current symptoms:
  - 5xx
  - hostname mismatch
  - stale preview
  - public route unreachable
- Verification checks:
  - HTTP status
  - expected hostname/route
  - health endpoint if available

## Core Rule
Roll back the smallest Cloudflare-facing change that restores service.

- Prefer reverting the bad route over making broad unrelated changes
- Prefer restoring a previously verified target over guessing a new one
- If no safe rollback target exists, stop and escalate instead of improvising

## Failure Domain Checklist
### DNS Route Failure
Examples:
- hostname points to the wrong tunnel
- hostname points to an old deployment
- preview hostname no longer matches the intended target

### Tunnel Failure
Examples:
- tunnel route exists but the backing local service is gone
- a newly created tunnel broke the working public preview path

### Cloudflare Deploy Route Failure
Examples:
- production or preview traffic points to an unhealthy deployment
- route switch succeeded technically but the new revision is broken

## Workflow
1. Confirm the incident is in the Cloudflare-facing layer, not only in the app.
2. Identify the smallest failing component:
   - DNS record
   - tunnel/public route
   - deployment route
3. Locate the last known good reference.
4. Choose the narrowest rollback action that can restore traffic safely.
5. Apply the rollback:
   - restore previous DNS target, or
   - switch back to the prior tunnel/public route, or
   - restore the previously verified deployment route.
6. Re-check the public entrypoint.
7. Record exactly what was rolled back and what still remains degraded, if
   anything.

## Rollback Order
When multiple Cloudflare components changed in one rollout, prefer this order:

1. public DNS route
2. tunnel/public preview mapping
3. deployment route

Reason: DNS or public routing mistakes usually restore faster than rebuilding or
redeploying application runtime.

## Decision Rules
| Situation | Result | Action |
|---|---|---|
| Last known good route exists and rollback restores service | `recovered` | Keep rollback, report success |
| Rollback partially helps but system is still degraded | `partial` | Keep safest recovered state and report remaining issue |
| No verified previous target exists | `blocked` | Escalate; do not guess |
| Failure is not in Cloudflare layer | `not_applicable` | Hand off to app/runtime troubleshooting |

## Guardrails
- Never roll back to an unverified or speculative target.
- Never change unrelated DNS records during incident response.
- Never hide a partial rollback behind a “success” label.
- Never keep flipping routes repeatedly without verifying after each change.
- If the issue is local runtime reachability, use preview/runtime recovery
  skills first.

## Verification Checklist
After rollback, verify at least:
- the public hostname resolves to the expected target
- the public URL returns the expected status code
- critical health or preview page loads again

If any of these still fail, the rollback is only partial.

## Output Contract
Emit:
- `cloudflare_rollback_executed`: `true` | `false`
- `cloudflare_rollback_target`
- `cloudflare_rollback_status`: one of
  - `recovered`
  - `partial`
  - `blocked`
  - `not_applicable`
- `cloudflare_rollback_reason`
- `cloudflare_post_rollback_verification`

## Good Output Example
```text
cloudflare_rollback_executed: true
cloudflare_rollback_target: preview.example.com -> previous tunnel target abc123.trycloudflare.com
cloudflare_rollback_status: recovered
cloudflare_rollback_reason: new_dns_route_pointed_to_wrong_preview_target
cloudflare_post_rollback_verification: GET https://preview.example.com returned 200 and served the expected preview page
```

## Blocked Output Example
```text
cloudflare_rollback_executed: false
cloudflare_rollback_target: none
cloudflare_rollback_status: blocked
cloudflare_rollback_reason: no_verified_last_known_good_cloudflare_target
cloudflare_post_rollback_verification: not_run
```

## Related Skills
- `rollback-deploy`
- `cloudflare-dns-route`
- `setup-cloudflare-tunnel`
- `deploy-precheck-cloudflare`
- `deploy-cancel-stop-cleanup`
