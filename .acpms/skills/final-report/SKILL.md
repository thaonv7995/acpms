---
name: final-report
description: Produce a short final report that highlights outcome, verification, and next action only when needed.
---

# Final Report

## Objective
Deliver a short completion report that a human can scan in a few seconds.

## When This Applies
- At the end of a successful or partially successful attempt
- After review handoff when ACPMS needs one final human-readable summary
- After deploy or preview flow when outcome and next action must be obvious

## Inputs
- Final change summary
- Relevant verification outcomes
- Deploy or preview status when applicable
- Real blocker or next action, if one exists

## Workflow
1. Collect only the high-signal facts from the attempt.
2. Convert them into two to four short bullets.
3. Drop repetitive transcript and low-signal detail.
4. Stop after the final report; do not restate it in extra prose.

## Decision Rules
| Situation | Action |
|---|---|
| Task is tiny or docs-only | Use two short bullets; do not inflate the report. |
| Verification failed for an unrelated baseline issue | Mention it once in `Next:` and tie it to scope. |
| No real risk or next step exists | Omit `Next:` entirely. |

## Output Contract
- Use `## Final Report`
- Prefer only `Done:`, `Verified:`, `Deploy:`, `Next:` when they add value
- Keep the report concise enough for the attempt timeline

## Related Skills
- `verify-test-build`
- `review-handoff`
- `release-note-and-delivery-summary`
- `update-deployment-metadata`
