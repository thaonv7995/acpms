---
name: cloudflare-config-validate
description: Validate Cloudflare settings and env prerequisites before any preview tunnel, DNS route, or Cloudflare deployment step. Use this before attempting public preview or Cloudflare publish flows.
---

# Cloudflare Config Validate

## Objective
Decide whether ACPMS should:

1. continue with Cloudflare-backed preview or deploy work,
2. continue with local preview only, or
3. stop and surface a clear reason to the user.

This skill is a guardrail. It does not create tunnels or DNS records. It only
checks whether the required Cloudflare configuration is present, internally
consistent, and appropriate for the next step.

## When This Applies
- A task needs a public preview URL instead of local-only preview
- A task needs Cloudflare tunnel setup
- A task needs DNS route creation or update
- A task needs Cloudflare Pages or Workers deployment
- ACPMS preview flow asks whether Cloudflare should be attempted

Do **not** use this skill for pure local preview flows that do not require a
public URL.

## Inputs
- ACPMS-injected env vars, when available:
  - `CLOUDFLARE_ACCOUNT_ID`
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - `CLOUDFLARE_BASE_DOMAIN`
- Project intent:
  - local preview only
  - public preview via Cloudflare tunnel
  - Cloudflare Pages deploy
  - Cloudflare Workers deploy
- Existing preview context, if any:
  - local preview URL
  - current tunnel/public URL
  - existing `.acpms/preview-output.json`

## Core Rule
Treat Cloudflare as **ready** only when the fields required for the current flow
are present.

- For local preview only:
  - Cloudflare config is optional
- For public preview via tunnel:
  - account ID, API token, zone ID, and base domain are all required
- For Cloudflare Pages or Workers deploy:
  - account ID and API token are required at minimum
  - zone ID and base domain are also required when the flow needs DNS or a
    public preview hostname

## Validation Checklist
### 1. Normalize Values
- Trim whitespace and newlines from all values
- Treat empty strings as missing
- Treat obviously masked values such as `••••` as invalid
- Never assume a value is usable just because the key exists

### 2. Validate By Flow
#### Local preview only
- No Cloudflare config is required
- Result may still be `cloudflare_not_required`

#### Public preview via tunnel
Require all of:
- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`
- `CLOUDFLARE_ZONE_ID`
- `CLOUDFLARE_BASE_DOMAIN`

#### Cloudflare Pages / Workers deploy
Require:
- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`

Also require:
- `CLOUDFLARE_ZONE_ID`
- `CLOUDFLARE_BASE_DOMAIN`

when the task also needs DNS routing or a public preview URL.

### 3. Sanity Check Base Domain
- Base domain must look like a hostname, not a URL
- Good:
  - `preview.example.com`
  - `apps.example.com`
  - `example.com`
- Bad:
  - `https://preview.example.com`
  - `http://example.com`
  - `/preview`

If the value looks like a URL, treat it as invalid configuration.

### 4. Sanity Check Flow Intent
- If the task only needs a local preview, do not block on missing Cloudflare
  settings
- If the task explicitly requires a public preview URL, do not silently degrade
  without surfacing a reason
- If Cloudflare config is partial, stop the Cloudflare step and tell the user
  exactly what is missing

## Workflow
1. Identify whether the next action is:
   - local preview only,
   - public preview via tunnel,
   - Pages deploy,
   - Workers deploy,
   - or DNS route management.
2. Read ACPMS-injected Cloudflare env vars.
3. Normalize all values by trimming whitespace and rejecting masked/empty
   placeholders.
4. Check the minimum required fields for the identified flow.
5. If base domain is required, confirm it is hostname-shaped, not URL-shaped.
6. Decide one of:
   - Cloudflare not required
   - Cloudflare ready
   - Cloudflare partially configured
   - Cloudflare not configured
7. Emit a user-facing message when config is missing or partial.
8. Return a machine-parseable status so downstream skills know whether to
   continue or stop.

## Decision Rules
| Situation | Result | Action |
|---|---|---|
| Task only needs local preview | `cloudflare_not_required` | Continue without Cloudflare |
| Account ID and token missing | `cloudflare_not_configured` | Stop Cloudflare step, tell user to configure Settings |
| Account ID/token present but zone/base domain missing for tunnel/public preview | `cloudflare_partial_config` | Stop Cloudflare step, explain missing fields |
| Base domain is URL-shaped or malformed | `cloudflare_invalid_config` | Stop and tell user to correct the value |
| All required fields for the current flow are present | `cloudflare_ready` | Continue to tunnel/DNS/deploy skill |

## Log for User
Use short, direct wording in the attempt log.

### Missing all Cloudflare config
`Cloudflare is not configured in System Settings (/settings). Configure Account ID, API Token, Zone ID, and Base Domain to enable public preview or Cloudflare deployment.`

### Partial config for tunnel/public preview
`Cloudflare settings are incomplete for public preview. Zone ID and Base Domain are required in System Settings (/settings).`

### Invalid base domain
`Cloudflare Base Domain must be a hostname like preview.example.com, not a full URL. Update it in System Settings (/settings).`

### Local preview only
`Cloudflare is not required for this local preview flow.`

## Output Contract
Emit:
- `cloudflare_config_status`: one of
  - `cloudflare_ready`
  - `cloudflare_partial_config`
  - `cloudflare_not_configured`
  - `cloudflare_invalid_config`
  - `cloudflare_not_required`
- `cloudflare_config_reason`: short machine-friendly explanation
- `cloudflare_missing_fields`: list when applicable
- `cloudflare_user_message`: exact human-facing message when applicable

## Good Output Example
```text
cloudflare_config_status: cloudflare_ready
cloudflare_config_reason: public_preview_requirements_present
cloudflare_missing_fields: []
```

## Partial Config Example
```text
cloudflare_config_status: cloudflare_partial_config
cloudflare_config_reason: missing_zone_and_base_domain_for_public_preview
cloudflare_missing_fields: ["CLOUDFLARE_ZONE_ID", "CLOUDFLARE_BASE_DOMAIN"]
cloudflare_user_message: Cloudflare settings are incomplete for public preview. Zone ID and Base Domain are required in System Settings (/settings).
```

## Bad Behavior Example
- Proceeding to tunnel creation when zone ID is missing
- Treating `https://preview.example.com` as a valid base domain
- Claiming Cloudflare is unavailable for a task that only needs local preview
- Failing silently without telling the user which fields are missing

## Related Skills
- `deploy-precheck-cloudflare`
- `setup-cloudflare-tunnel`
- `create-cloudflare-preview-tunnel`
- `cloudflare-dns-route`
- `deploy-cloudflare-pages`
- `deploy-cloudflare-workers`
