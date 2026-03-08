---
name: follow-up-execution-strategy
description: Decide whether a follow-up should reuse the current attempt, create a new attempt, reuse the same MR, or open a new MR based on task and review state.
---

# Follow Up Execution Strategy

## Objective
Choose the correct continuation strategy for follow-up work so ACPMS does not
mix incompatible states between merged, in-review, failed, and still-running
attempts.

## When This Applies
- User sends follow-up instructions
- A task already has an attempt history
- ACPMS must decide whether to continue same attempt or create a new one
- MR reuse vs new MR is part of the decision

## Inputs
- Current task status
- Latest attempt status
- Review/merge state
- Existing MR state
- Whether the previous worktree still exists and is reusable

## Workflow
1. Identify the current lifecycle state:
   - running
   - in review
   - merged/done
   - failed/cancelled
2. Decide whether same-attempt continuation is still valid.
3. Decide whether the existing MR should be reused or a new MR should be
   created.
4. Pick the safest branch/worktree strategy.
5. Explain the chosen continuation policy in machine-readable terms.

## Decision Rules
| Situation | Action |
|---|---|
| Attempt is still running | Send follow-up into the same attempt |
| Attempt is in review and not merged | Reuse the same attempt/MR when safe |
| Task is already merged/done | Create a new attempt and a new MR |
| Previous attempt is failed but branch/worktree is reusable | Reuse only if it is safe and intentional |

## Output Contract
Emit:
- `followup_strategy`: `same_attempt` | `new_attempt`
- `mr_strategy`: `reuse` | `new_mr` | `none`
- `followup_reason`
- `expected_branch_strategy`

## Related Skills
- `worktree-branch-recovery`
- `review-handoff`
- `gitlab-merge-request`

