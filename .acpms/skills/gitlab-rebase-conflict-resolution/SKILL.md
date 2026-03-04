---
name: gitlab-rebase-conflict-resolution
description: Resolve branch divergence and rebase conflicts safely before retrying push or merge actions. Use when Request Changes feedback asks to resolve merge conflicts or pull main and rebase.
---

# GitLab Rebase Conflict Resolution

## Objective
Recover from non-fast-forward and rebase conflicts while preserving intended task changes.

## Inputs
- Current branch and target base branch.
- Conflict files and git conflict markers.
- Task intent and accepted scope.

## Workflow
1. Fetch latest target branch: `git fetch origin`
2. **Always** rebase onto base: `git rebase origin/main` (or target branch). Do NOT skip rebase.
   - "Already up to date" from fetch means no new objects were downloaded—it does NOT mean your branch is integrated with main. Rebase anyway.
   - If branch is missing content that main has (e.g. landing page), rebase will bring it in; resolve any conflicts.
3. Resolve conflicts file-by-file using task intent as tie-breaker.
4. Run focused verification on conflict-affected areas.
5. Continue rebase and push updated branch: `git push --force-with-lease origin HEAD` (if rebased).

## Decision Rules
| Situation | Action |
|---|---|
| "Already up to date" from fetch | Still run rebase. Fetch output ≠ branch integrated with main. |
| Branch missing files that main has | Rebase brings them in; resolve conflicts, keep task changes. |
| Conflict touches unrelated domain | Prefer base branch behavior unless task requires change. |
| Conflict cannot be resolved confidently | Stop and mark manual resolution required. |
| Rebase completes but tests fail | Keep branch unmerged and report failed verification. |

## Guardrails
- Do not force-push without stating why.
- Do not drop task-required hunks during conflict resolution.
- **Do NOT suggest** "refresh/retry on GitLab" or "send me the error"—you must run the commands and resolve conflicts yourself.

## Output Contract
Include:
- `rebase_status`: `resolved` | `blocked` | `failed`
- `conflict_files`
- `resolution_notes`
- `post_rebase_verification`
