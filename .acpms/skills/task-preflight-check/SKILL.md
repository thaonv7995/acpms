---
name: task-preflight-check
description: Validate references and environment before execution. Block and report if prerequisites are missing.
---

# Task Preflight Check

## Objective
Run first before implementation, deploy, or retry work. Confirm that the repo,
references, and execution environment are usable. If a blocking issue exists,
stop immediately so ACPMS does not waste the attempt on predictable failure.

## When This Applies
- Every task execution, including init, follow-up, retry, and deploy flows
- Before `code-implement`, `init-project-bootstrap`, preview/deploy work, or
  any expensive verification step

## Inputs
- Task description and any mention of uploaded files or attachments
- `.acpms/references/refs_manifest.json` and files under `.acpms/references/`
  when present
- Current git/worktree state
- Project manifests such as `package.json`, `Cargo.toml`, `pyproject.toml`,
  `Dockerfile`, or other runtime hints when relevant

## Workflow
1. Check whether the task depends on references or uploaded files.
2. Validate reference manifests and referenced files if they exist.
3. Validate the repository state:
   - `git status` works
   - the worktree is not broken
   - no merge conflict or detached-head state blocks progress
4. Check for obvious environment blockers that would make the next skill fail
   immediately.
5. Either stop with a clear preflight block, or continue silently.

## Decision Rules
| Situation | Action |
|---|---|
| Reference download failures exist | Stop and emit a preflight block. |
| Referenced files are missing or unreadable | Stop and emit a preflight block. |
| Repository is broken or unusable | Stop and emit a preflight block. |
| Advisory issue only | Warn only if useful; do not block the task. |
| All checks pass | Continue silently to the next skill. |

## Output Contract
If blocking, emit:

```md
## PREFLIGHT BLOCKED

Task cannot proceed. Please resolve the following before retrying:

### Issues
- ...

### Suggested Actions
- ...
```

If passing, produce no extra chatter and hand off to the next skill.

## Related Skills
- `env-and-secrets-validate`
- `code-implement`
- `verify-test-build`
- `retry-triage-and-recovery`
