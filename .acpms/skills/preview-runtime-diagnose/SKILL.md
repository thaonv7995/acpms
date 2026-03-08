---
name: preview-runtime-diagnose
description: Diagnose why a preview URL is stale, unreachable, serving the wrong code, or mismatched with its recorded metadata before attempting repair.
---

# Preview Runtime Diagnose

## Objective
Identify the real failure point in a preview flow before changing files,
restarting containers, or rewriting runtime metadata.

This skill is diagnostic-first. Use it when preview appears broken, stale, or
misleading, and ACPMS needs to know whether the issue is in the app, the
runtime, the port binding, the contract file, or stale metadata.

## When This Applies
- Preview URL exists but does not load
- Preview panel shows stale code
- `PREVIEW_TARGET` exists but curl/browser verification fails
- A preview URL works in logs but not in the UI
- Stop/restart behavior is inconsistent and root cause is unclear

## Inputs
- `PREVIEW_TARGET` and `PREVIEW_URL`, if present
- `.acpms/preview-output.json`, if present
- Runtime control metadata such as:
  - compose project name
  - container name
  - port
  - process id
- Current worktree path and attempt context

## Workflow
1. Read the preview contract and extract the expected local target.
2. Check whether the worktree still exists.
3. Check whether the expected local port is listening.
4. Check whether the expected container or compose project exists.
5. Curl the local preview URL and inspect the response.
6. Compare the live runtime with `.acpms/preview-output.json` and attempt
   metadata.
7. Classify the failure domain before proposing any repair.

## Decision Rules
| Situation | Action |
|---|---|
| Worktree missing but preview metadata still exists | Mark stale metadata/runtime mismatch |
| Port not listening | Mark runtime not running |
| Port listens but response is wrong | Mark wrong runtime or stale artifact |
| Contract file differs from actual runtime | Mark preview contract drift |
| Everything matches and URL still fails only in UI | Mark UI/client-side issue |

## Log for User
| Condition | Message |
|---|---|
| Preview metadata is stale | `The recorded preview no longer matches a live runtime. I need to repair or recreate the preview.` |
| Runtime is down | `The preview runtime is not currently running. I need to restart or recreate it.` |
| Runtime is serving the wrong content | `The preview runtime is up, but it is serving stale or unexpected output. I need to rebuild or replace it.` |

## Output Contract
Emit:
- `preview_diagnosis_status`: `healthy` | `stale` | `down` | `drifted` | `wrong_content` | `ui_only`
- `preview_failure_domain`
- `preview_repair_recommendation`
- `preview_observations`

## Related Skills
- `preview-contract-repair`
- `preview-docker-runtime`
- `docker-compose-repair`
- `deployment-runtime-ownership`

