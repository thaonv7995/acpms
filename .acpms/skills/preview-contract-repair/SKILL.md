---
name: preview-contract-repair
description: Repair `.acpms/preview-output.json` and related preview metadata when local target, public URL, or runtime control fields are missing, malformed, or inconsistent.
---

# Preview Contract Repair

## Objective
Restore a valid ACPMS preview contract so stop/restart/reload behavior can work
reliably and the UI can render the correct preview URL.

## When This Applies
- `.acpms/preview-output.json` is missing required fields
- `PREVIEW_TARGET` and `PREVIEW_URL` were concatenated or malformed
- `runtime_control` is missing even though the runtime is controllable
- Local target and public URL are swapped
- A follow-up needs to stop or restart a runtime but the contract is incomplete

## Inputs
- Existing `.acpms/preview-output.json`
- Actual live runtime details:
  - local URL
  - public URL, if one exists
  - container or compose ownership
- ACPMS preview contract rules

## Workflow
1. Read the current preview contract file.
2. Normalize `preview_target` and `preview_url` according to ACPMS rules.
3. Restore or add `runtime_control` when the runtime is actually stoppable.
4. Remove malformed, duplicated, or concatenated text artifacts.
5. Rewrite `.acpms/preview-output.json` as clean JSON.
6. Re-emit `PREVIEW_TARGET` and `PREVIEW_URL` from the repaired contract.

## Decision Rules
| Situation | Action |
|---|---|
| Only local runtime exists | Set both `preview_target` and `preview_url` to the local URL |
| Public URL exists | Keep local URL in `preview_target`, public URL in `preview_url` |
| Runtime is Docker Compose controlled | Add `runtime_control` with compose project metadata |
| Runtime is uncontrollable or unknown | Leave `runtime_control` absent and say so |

## Guardrails
- Never put a public URL into `preview_target`
- Never keep concatenated markers such as `PREVIEW_URL:` inside URL fields
- Never invent runtime control metadata you cannot verify

## Output Contract
Write `.acpms/preview-output.json` with:
- `preview_target`
- `preview_url`
- `runtime_control` when verifiable

Also emit:
- `PREVIEW_TARGET: ...`
- `PREVIEW_URL: ...`

## Related Skills
- `preview-runtime-diagnose`
- `preview-docker-runtime`
- `create-cloudflare-preview-tunnel`
- `update-deployment-metadata`

