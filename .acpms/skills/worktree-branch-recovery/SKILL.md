---
name: worktree-branch-recovery
description: Recover from stale worktree, branch already exists, detached state, or broken follow-up branch/worktree wiring before execution continues.
---

# Worktree Branch Recovery

## Objective
Repair attempt branch and worktree state without losing task changes or making
Git state worse.

## When This Applies
- Follow-up fails with `branch already exists`
- Worktree path is missing, stale, or invalid
- Attempt branch exists but is not attached to a valid worktree
- Git state is detached or blocked before agent execution can continue

## Inputs
- Attempt id
- Expected worktree path
- Expected attempt branch name
- Current git/worktree state

## Workflow
1. Inspect whether the expected worktree path exists and is valid.
2. Inspect whether the attempt branch exists locally or remotely.
3. Determine whether the branch is already attached to a valid worktree.
4. Reattach or recreate the worktree using the existing attempt branch when
   safe.
5. If stale directories remain, clean only the stale attempt-owned paths.
6. Leave the repo in a state where the follow-up can continue on the correct
   branch.

## Decision Rules
| Situation | Action |
|---|---|
| Branch exists and worktree is missing | Recreate worktree on that branch |
| Worktree exists but is invalid | Clean and recreate it |
| Detached or broken git state blocks progress | Repair to the expected attempt branch |
| Recovery would risk unrelated local work | Stop and escalate |

## Log for User
| Condition | Message |
|---|---|
| Existing attempt branch is being reused | `I found the existing attempt branch and am reattaching the worktree so the follow-up can continue safely.` |
| Stale worktree was cleaned | `I cleaned up stale attempt worktree state and recreated the working branch context.` |
| Recovery is blocked | `The attempt branch/worktree state could not be recovered safely. Manual git review is needed before continuing.` |

## Output Contract
Emit:
- `worktree_recovery_status`: `reused` | `recreated` | `blocked`
- `worktree_recovery_reason`
- `recovered_branch_name`
- `recovered_worktree_path`

## Related Skills
- `retry-triage-and-recovery`
- `gitlab-rebase-conflict-resolution`
- `follow-up-execution-strategy`

