---
name: review-handoff
description: Prepare complete handoff package when require-review mode is enabled and commit/push is deferred.
---

# Review Handoff

## Objective
Hand over task output to human review with enough evidence to approve safely.

## Inputs
- Task output in worktree.
- Verification results.
- Review-required policy state.

## Workflow
1. Ensure code changes are complete and scoped.
2. Do not commit or push in review-required mode.
3. Prepare reviewer summary with changed files and rationale.
4. Highlight risks, TODOs, and validation gaps.
5. Mark clear approval instructions.

## Decision Rules
| Situation | Action |
|---|---|
| Verification incomplete due environment limits | Report exact gap and impact before handoff. |
| Unrelated diff exists | Exclude from handoff scope and call out explicitly. |
| Follow-up required before merge | Mark as `needs_followup` with owner/action. |

## Guardrails
- Never claim release-ready if essential checks were skipped.
- Do not include hidden assumptions.

## Output Contract
Include:
- `handoff_status`: `ready_for_review` | `needs_followup`
- `changed_files`
- `verification_summary`
- `known_risks`
- `reviewer_actions`
