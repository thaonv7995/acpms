---
name: gitlab-issue-sync
description: Sync task execution outcome back to related GitLab issue with clear status and evidence.
---

# GitLab Issue Sync

## Objective
Keep issue tracking accurate by posting outcome, links, and next actions.

## Inputs
- Related issue reference (if available in task metadata/context).
- Task execution report.
- MR/deploy links.

## Workflow
1. Resolve linked issue reference.
2. Post structured update comment.
3. Include MR link, deployment status, and verification summary.
4. If blocked/skipped, include exact blocker and required owner action.

## Comment Structure
- `Status`
- `What changed`
- `Verification`
- `Deployment`
- `Links`
- `Blockers/Next steps`

## Decision Rules
| Situation | Action |
|---|---|
| No linked issue | Skip and report `issue_sync_skipped_no_issue`. |
| Issue is closed but task still active | Post note without reopening unless policy requires. |

## Output Contract
Include:
- `issue_sync_status`: `posted` | `skipped` | `failed`
- `issue_sync_reason`
- `issue_ref`
