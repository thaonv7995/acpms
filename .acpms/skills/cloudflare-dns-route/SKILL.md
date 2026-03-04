---
name: cloudflare-dns-route
description: Create or validate Cloudflare DNS route records for preview or production service endpoints.
---

# Cloudflare DNS Route

## Objective
Ensure service hostname resolves correctly to deployed target.

## Inputs
- Zone ID/base domain.
- Hostname/subdomain policy.
- Target endpoint (Pages/Workers/tunnel).

## Workflow
1. Determine desired DNS record type and target.
2. Create or update DNS record idempotently.
3. Verify resolution and routing health.
4. Report record details.

## Decision Rules
| Situation | Action |
|---|---|
| Record already correct | Reuse and report as unchanged. |
| Record exists but incorrect target | Update record and verify. |
| Missing zone/domain config | Skip and report required settings. |

## Output Contract
Include:
- `dns_route_status`: `active` | `updated` | `skipped` | `failed`
- `dns_hostname`
- `dns_record_type`
- `dns_target`
