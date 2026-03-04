---
name: cloudflare-config-validate
description: Validate Cloudflare account/token/domain prerequisites before tunnel or deployment actions. Agent must log user-friendly message when validation fails (see Log for User).
---

# Cloudflare Config Validate

## Objective
Prevent invalid Cloudflare operations by validating config prerequisites first. When validation fails, **agent must output a message** for the user in the attempt log.

## Inputs
- Cloudflare account ID availability.
- Cloudflare API token availability.
- Optional zone/base domain for DNS/tunnel paths.
- Project type and auto-deploy flag.

## Workflow
1. Check account ID and API token are present and non-empty.
2. For DNS/tunnel flows, also check zone/base-domain requirements.
3. Return readiness result and stop unsafe operations if not ready.
4. When not ready: **output Log for User message**.

## Log for User
**Agent must output these messages** when validation fails—they appear in the attempt log (chat session).

| Condition | Message to output |
|-----------|-------------------|
| account+token missing | Cloudflare is not configured. Configure in System Settings (/settings) to enable preview. |
| zone/base-domain missing (for DNS) | Cloudflare Zone ID and Base Domain are required for tunnel preview. Configure in System Settings (/settings). |

## Decision Rules
| Condition | Result |
|---|---|
| account+token missing | `cloudflare_not_configured` + Log for User message |
| account+token present, zone missing for DNS | `cloudflare_partial_config` + Log for User message |
| required fields present | `cloudflare_ready` |

## Output Contract
Include:
- `cloudflare_config_status`: `cloudflare_ready` | `cloudflare_partial_config` | `cloudflare_not_configured`
- `cloudflare_config_reason`
