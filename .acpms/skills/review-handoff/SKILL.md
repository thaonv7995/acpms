---
name: review-handoff
description: Prepare complete handoff package when require-review mode is enabled and commit/push is deferred.
---

# Review Handoff

## Objective
Hand over task output to human review with enough evidence to approve safely,
without pretending the task is fully merge-ready when key gaps remain.

## When This Applies
- `require_review` is enabled and ACPMS should not auto-complete by merging or pushing further
- The task is ready for human review but still needs a clear handoff summary
- Verification is complete enough to review, even if one or two explicit gaps remain

## Inputs
- Task output in the worktree
- Verification results
- Review-required policy state

## Workflow
1. Ensure the change set is complete and scoped.
2. Do not commit or push in review-required mode unless the workflow explicitly allows it.
3. Prepare a reviewer summary with changed files and rationale.
4. Highlight known risks, TODOs, and validation gaps.
5. State the reviewer action needed to move forward.

## Decision Rules
| Situation | Action |
|---|---|
| Verification is incomplete due to environment limits | Report the exact gap and impact before handoff. |
| Unrelated diff exists | Exclude it from handoff scope and call it out explicitly. |
| Follow-up is required before merge | Mark `needs_followup` with a concrete action. |

## Output Contract
Emit:
- `handoff_status`: `ready_for_review` | `needs_followup`
- `changed_files`
- `verification_summary`
- `known_risks`
- `reviewer_actions`

## Related Skills
- `verify-test-build`
- `gitlab-merge-request`
- `final-report`
- `update-deployment-metadata`
