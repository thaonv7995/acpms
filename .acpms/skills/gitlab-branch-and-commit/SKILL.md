---
name: gitlab-branch-and-commit
description: Execute safe Git/GitLab branch, staging, commit, and push workflow for task delivery.
---

# GitLab Branch And Commit

## Objective
Create a clean, traceable Git history for task delivery without leaking
unrelated changes or staging ACPMS internal files by accident.

## When This Applies
- ACPMS is ready to stage, commit, and push task-scoped changes
- The workflow allows commit/push for the current attempt
- Branch state must be normalized before MR/PR creation

## Inputs
- Task context and touched files
- Current branch and git status
- Review policy (`require_review`)
- Remote branch expectations for the current attempt

## Workflow
1. Inspect `git status --short` and identify only task-relevant files.
2. Ensure the current branch is safe for this task.
3. Stage only intended files.
4. Commit with a clear, scoped message.
5. Push to the correct remote branch.

## Decision Rules
| Situation | Action |
|---|---|
| Review mode forbids commit/push | Do not commit or push; report ready-for-review state. |
| Unrelated modified files exist | Exclude them from staging and mention them in the report if needed. |
| Push is rejected | Re-sync safely or stop and report branch divergence. |

## Guardrails
- Never use `git add .` when unrelated changes are present.
- Never stage `.acpms/`, `node_modules/`, `dist/`, `target/`, `.env*`, logs, or IDE artifacts.
- Never rewrite shared branch history unless explicitly requested.

## Output Contract
Emit:
- `git_branch`
- `git_commit`
- `git_push`: `success` | `skipped` | `failed`
- `git_push_reason` when skipped or failed

## Related Skills
- `verify-test-build`
- `gitlab-merge-request`
- `review-handoff`
- `release-note-and-delivery-summary`
