---
name: task-preflight-check
description: Validate references and environment before execution. Block and report if prerequisites are missing.
---

# Task Preflight Check

## Objective
Run **first** before any implementation work. Verify that references are available and the environment is ready. If any blocking issue exists, **stop immediately** and report to the user so the task can be fixed before retrying.

## When This Applies
- Every task execution (init and non-init)
- Must run before code-implement, init-project-bootstrap, or any other implementation skill

## Workflow

### 1. Check References (if task has attachments)
- If `.acpms/references/refs_manifest.json` exists:
  - Read the manifest.
  - If `failures` array is non-empty → **BLOCK**. Output preflight report and stop.
  - If `files` array lists files → verify each file exists and is readable.
  - If any listed file is missing or unreadable → **BLOCK**. Output preflight report and stop.
- If task description mentions attachments but `.acpms/references/` is missing or empty → **BLOCK**.

### 2. Check Environment
- Verify current directory is a valid git repository (`git status` succeeds).
- Verify worktree is not in a broken state (e.g. merge conflicts, detached HEAD without clear branch).
- If project has `package.json` / `Cargo.toml` / similar: verify lockfile or dependency manifest exists (optional, warn only).

### 3. Decision
| Situation | Action |
|-----------|--------|
| All checks pass | Proceed with implementation. No output needed. |
| Reference download failures | **STOP**. Output preflight report. |
| Reference files missing/unreadable | **STOP**. Output preflight report. |
| Git repo broken or missing | **STOP**. Output preflight report. |
| Merge conflicts or dirty state that blocks work | **STOP**. Output preflight report. |

## Preflight Report Format (when blocking)

When blocking, output a clear section:

```
## PREFLIGHT BLOCKED

Task cannot proceed. Please resolve the following before retrying:

### Issues
- [List each blocking issue with actionable detail]

### Suggested Actions
- [What the user should do to fix each issue]
```

Include this in the final report so the user is notified that the task has issues requiring resolution before execution.

## Output Contract
- If blocking: emit preflight report and **do not** proceed with implementation.
- If passing: continue to next skill silently.
