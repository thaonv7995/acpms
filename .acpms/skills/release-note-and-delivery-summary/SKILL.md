---
name: release-note-and-delivery-summary
description: Produce a concise, reusable delivery summary that can feed MR descriptions, issue updates, task completion notes, and handoff communications.
---

# Release Note And Delivery Summary

## Objective
Write one concise delivery summary that other ACPMS steps can reuse instead of
repeating different versions of the same outcome across MRs, issues, and final
reports.

## When This Applies
- A task finished with meaningful delivery output
- A merge request, issue comment, or handoff note needs a concise summary
- ACPMS needs one canonical delivery block for downstream reuse

## Inputs
- What changed
- Verification results
- Deployment or preview status
- Known risks and next steps

## Workflow
1. Summarize the actual user-visible or operator-visible change.
2. Capture the most important verification outcomes.
3. Add deploy/preview status only when relevant.
4. Add follow-up risk only if it matters for the next reader.
5. Keep the final summary compact and reusable.

## Decision Rules
| Situation | Action |
|---|---|
| Docs-only or small change | Keep summary very short |
| Verification has a pre-existing failure | Mention it only if it affects trust in delivery |
| No deploy occurred | Say so briefly or omit deploy section entirely |

## Output Contract
Emit:
- `delivery_status`: `complete` | `partial` | `blocked`
- `user_impact`
- `technical_impact`
- `verification_highlights`
- `deployment_highlights`
- `followups`

## Related Skills
- `final-report`
- `review-handoff`
- `gitlab-issue-sync`
