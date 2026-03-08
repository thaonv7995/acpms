---
name: deployment-runtime-ownership
description: Classify whether a preview or deployment runtime is ACPMS-managed, agent-managed, external, or stale so start/stop/rebuild actions use the right control path.
---

# Deployment Runtime Ownership

## Objective
Prevent ACPMS and the agent from sending `stop`, `restart`, or `rebuild` to the
wrong runtime by first deciding who actually owns and can control it.

## When This Applies
- Preview exists but stop/restart semantics are unclear
- ACPMS must choose between hard flow and agent follow-up fallback
- Runtime control metadata is incomplete or suspect

## Inputs
- `.acpms/preview-output.json`
- `runtime_control` metadata
- existing preview deployment records
- live container/compose/process observations

## Workflow
1. Read the preview contract and runtime control metadata.
2. Determine whether the runtime is:
   - ACPMS-managed
   - agent-managed but controllable
   - external/uncontrollable
   - stale/orphaned
3. Match UI actions and stop/restart behavior to that ownership model.
4. Report the ownership result so the caller picks the right action path.

## Decision Rules
| Situation | Action |
|---|---|
| ACPMS-managed preview record exists and matches runtime | Use hard control path |
| Agent-managed runtime has valid `runtime_control` | Use contract-based control path |
| Only URL exists and no control metadata exists | Treat as external or dismiss-only |
| Runtime no longer exists but metadata remains | Treat as stale/orphaned |

## Output Contract
Emit:
- `runtime_ownership`: `acpms_managed` | `agent_managed` | `external` | `stale`
- `runtime_control_capability`
- `runtime_ownership_reason`

## Related Skills
- `preview-runtime-diagnose`
- `preview-contract-repair`
- `deploy-cancel-stop-cleanup`

