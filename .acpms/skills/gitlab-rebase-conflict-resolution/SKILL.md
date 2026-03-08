---
name: gitlab-rebase-conflict-resolution
description: Resolve branch divergence and rebase conflicts safely before retrying push or merge, while preserving the real task intent and verifying the result afterward.
---

# GitLab Rebase Conflict Resolution

## Objective
Recover from branch divergence, merge conflicts, and non-fast-forward push
errors without dropping task-required changes or force-pushing blindly.

## When This Applies
- Push is rejected because the branch is behind
- Merge request requires rebase
- Git reports conflict markers during rebase
- Request changes explicitly asks to rebase or resolve conflicts

## Inputs
- Current branch
- Target base branch, usually `main`
- Conflict files
- Task intent and accepted scope

## Workflow
1. Fetch the latest target branch.
2. Rebase onto the target base branch even if fetch says “already up to date”.
3. Resolve conflicts file by file using task intent as the tie-breaker.
4. Run focused verification on conflict-affected areas.
5. Continue the rebase and push with `--force-with-lease` only when the history
   was rewritten intentionally.

## Decision Rules
| Situation | Action |
|---|---|
| Branch is behind but conflict-free | Rebase and continue |
| Conflict touches unrelated code | Prefer base branch behavior unless task requires otherwise |
| Conflict cannot be resolved confidently | Stop and mark blocked |
| Rebase succeeds but verification fails | Keep branch unmerged and report failed verification |

## Guardrails
- Never use plain `--force`; use `--force-with-lease`
- Never drop task-required hunks during conflict resolution
- Never claim “already up to date” means no rebase is needed

## Output Contract
Emit:
- `rebase_status`: `resolved` | `blocked` | `failed`
- `conflict_files`
- `resolution_notes`
- `post_rebase_verification`

## Related Skills
- `gitlab-branch-and-commit`
- `gitlab-merge-request`
- `verify-test-build`
