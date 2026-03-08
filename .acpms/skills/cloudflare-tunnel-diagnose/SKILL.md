---
name: cloudflare-tunnel-diagnose
description: Diagnose why a Cloudflare public preview URL or tunnel route failed even though local preview may still be healthy.
---

# Cloudflare Tunnel Diagnose

## Objective
Determine whether a broken public preview is caused by local runtime, missing
Cloudflare config, tunnel creation failure, DNS mismatch, or stale public route.

## When This Applies
- Local preview works but public URL does not
- Tunnel creation failed or produced an unusable URL
- Cloudflare URL exists but points to the wrong preview
- ACPMS needs a root-cause report for tunnel failure

## Inputs
- Local preview URL
- Public preview URL, if present
- Cloudflare config in env
- DNS/route details if available
- Existing preview contract

## Workflow
1. Verify the local preview runtime first.
2. Check Cloudflare config completeness and basic shape.
3. Verify whether a public URL was actually created.
4. Compare the public route with the expected tunnel/DNS target.
5. Curl the public URL and classify the failure.
6. Decide whether the issue is local runtime, Cloudflare config, tunnel setup,
   DNS routing, or stale metadata.

## Decision Rules
| Situation | Action |
|---|---|
| Local preview is down | Do not blame Cloudflare first |
| Cloudflare config is incomplete | Mark config issue |
| Public URL missing | Mark tunnel creation issue |
| Public URL exists but wrong target | Mark DNS/route drift |
| Public URL exists but local preview is healthy and tunnel still fails | Mark tunnel/provider issue |

## Log for User
| Condition | Message |
|---|---|
| Cloudflare config missing | `Cloudflare is not fully configured, so public preview could not be created. Local preview may still be available.` |
| Tunnel failed after local preview succeeded | `The local preview is healthy, but Cloudflare public preview could not be established. I need to inspect tunnel or DNS routing.` |
| Public URL is stale | `The public preview URL no longer points to the correct runtime. I need to refresh the Cloudflare route.` |

## Output Contract
Emit:
- `cloudflare_tunnel_diagnosis`
- `cloudflare_failure_domain`
- `cloudflare_repair_recommendation`
- `cloudflare_local_preview_status`

## Related Skills
- `cloudflare-config-validate`
- `setup-cloudflare-tunnel`
- `create-cloudflare-preview-tunnel`
- `cloudflare-dns-route`

