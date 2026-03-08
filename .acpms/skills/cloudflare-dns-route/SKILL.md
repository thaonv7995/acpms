---
name: cloudflare-dns-route
description: Create or update the Cloudflare DNS record that exposes a preview or deployment hostname. Use after target runtime or tunnel details are already known.
---

# Cloudflare DNS Route

## Objective
Make a hostname resolve to the correct preview or deployment target without
creating duplicate, stale, or misleading DNS records.

This skill is responsible for the DNS record only. It does **not** validate
Cloudflare config from scratch and it does **not** bring up the local preview
runtime. Use it only after the target endpoint is already known.

## When This Applies
- A Cloudflare preview tunnel already exists and needs a DNS hostname
- A preview flow needs a stable hostname under the configured base domain
- A deploy flow needs a DNS route pointed at an already-created Cloudflare
  target
- A previous DNS record exists and may need idempotent reuse or update

Do **not** use this skill when:
- local preview is enough and no public hostname is needed
- Cloudflare configuration has not been validated yet
- the tunnel or target endpoint does not exist yet

## Inputs
- `CLOUDFLARE_ZONE_ID`
- `CLOUDFLARE_BASE_DOMAIN`
- A concrete target to point DNS at, such as:
  - a Cloudflare tunnel route
  - a Pages hostname
  - a Workers route target when applicable
- Route intent:
  - preview hostname
  - deployment hostname
  - temporary probe hostname
- Existing route information, if any:
  - current hostname
  - existing DNS record type
  - existing record target

## Core Rule
Only create or update DNS after the target is already real and known.

- No target -> no DNS write
- Invalid base domain -> no DNS write
- Existing correct record -> reuse it
- Existing wrong record -> update only if it belongs to this ACPMS flow and the
  replacement is intentional

## Route Shape Rules
### Preview Routes
- Prefer a subdomain under `CLOUDFLARE_BASE_DOMAIN`
- Use a host that is clearly preview-scoped and safe to replace
- Do not overwrite unrelated user-managed DNS records

### Record Type
- Prefer `CNAME` for preview routes that point at a tunnel or Cloudflare-managed
  hostname
- Use proxied mode when the flow expects Cloudflare to front the route
- Do not invent record types when the target is only known as a hostname

### Base Domain
- Must be a hostname such as:
  - `preview.example.com`
  - `apps.example.com`
  - `example.com`
- Must not be a URL such as:
  - `https://preview.example.com`
  - `http://example.com`

## Workflow
1. Confirm Cloudflare config for DNS is already available and validated.
2. Confirm the DNS target is already known and usable.
3. Derive the desired hostname under `CLOUDFLARE_BASE_DOMAIN`.
4. Decide the record type, normally `CNAME` for preview/public Cloudflare
   routes.
5. Check for an existing DNS record for the hostname.
6. If the existing record already matches the desired type and target, reuse it.
7. If the existing record belongs to this flow but targets the wrong value,
   update it idempotently.
8. If the existing record appears unrelated or risky to overwrite, stop and
   surface a failure reason instead of clobbering it.
9. Verify the resulting DNS record details returned by Cloudflare.
10. Emit the machine-readable route metadata for downstream steps.

## Validation Checklist
- Zone ID is present
- Base domain is present and hostname-shaped
- Target hostname/value is present
- Desired hostname is under the intended base domain
- Existing record, if any, is either:
  - already correct,
  - safe to update,
  - or must be treated as a conflict

## Decision Rules
| Situation | Result | Action |
|---|---|---|
| Zone ID or base domain missing | `skipped` | Stop and surface missing config |
| Target endpoint missing | `failed` | Stop; do not create placeholder DNS |
| Existing record already matches | `active` | Reuse and report unchanged |
| Existing record differs but is safe to replace | `updated` | Update record and report new target |
| Existing record conflicts and looks unrelated | `failed_conflict` | Stop and tell user which hostname is blocked |
| No record exists yet | `created` | Create the record and report details |

## Guardrails
- Never create DNS records before the tunnel or deployment target exists.
- Never write placeholder targets.
- Never overwrite a hostname that appears user-managed without an explicit
  reason from the task flow.
- Never claim a preview is public until the DNS record has been created or
  confirmed.
- Keep the hostname and target machine-parseable.

## Log for User
### Missing DNS config
`Cloudflare DNS routing is not configured. Zone ID and Base Domain are required in System Settings (/settings).`

### Missing target
`Cloudflare DNS route could not be created because the preview or deployment target is not ready yet.`

### DNS conflict
`Cloudflare DNS route was not changed because the hostname is already used by another record. Review the existing DNS entry before retrying.`

## Output Contract
Emit:
- `dns_route_status`: one of
  - `created`
  - `active`
  - `updated`
  - `skipped`
  - `failed`
  - `failed_conflict`
- `dns_hostname`
- `dns_record_type`
- `dns_target`
- `dns_proxied`
- `dns_route_reason`

## Good Output Example
```text
dns_route_status: created
dns_hostname: task-abc123.preview.example.com
dns_record_type: CNAME
dns_target: abc123-xyz.trycloudflare.com
dns_proxied: true
dns_route_reason: preview_cname_created
```

## Reuse Output Example
```text
dns_route_status: active
dns_hostname: task-abc123.preview.example.com
dns_record_type: CNAME
dns_target: abc123-xyz.trycloudflare.com
dns_proxied: true
dns_route_reason: existing_record_already_correct
```

## Bad Behavior Example
- Creating DNS before the tunnel URL exists
- Replacing a record for an unrelated hostname without warning
- Using a full URL as `CLOUDFLARE_BASE_DOMAIN`
- Claiming success without returning hostname and target

## Related Skills
- `cloudflare-config-validate`
- `deploy-precheck-cloudflare`
- `setup-cloudflare-tunnel`
- `create-cloudflare-preview-tunnel`
- `update-deployment-metadata`
