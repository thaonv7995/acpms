---
name: gitlab-issue-sync
description: Post a structured status update back to the linked GitLab issue so issue tracking reflects what really happened in the task.
---

# GitLab Issue Sync

## Objective
Keep the linked GitLab issue aligned with the actual task outcome, including
what changed, what was verified, and what remains blocked.

## When This Applies
- The task is linked to a GitLab issue
- A meaningful delivery or failure update should be posted back to the issue

## Inputs
- Linked issue reference
- Task outcome
- MR link, preview link, or deploy result when available

## Workflow
1. Resolve the linked GitLab issue.
2. Build a short structured update with status, changes, verification, and
   links.
3. If the task is blocked or partial, include the exact blocker and next action.
4. Post the comment and record whether sync succeeded.

## Decision Rules
| Situation | Action |
|---|---|
| No linked issue exists | Skip sync |
| Issue is linked and update is ready | Post comment |
| GitLab post fails | Mark sync failed and report why |

## Output Contract
Emit:
- `issue_sync_status`: `posted` | `skipped` | `failed`
- `issue_sync_reason`
- `issue_ref`

## Related Skills
- `gitlab-merge-request`
- `release-note-and-delivery-summary`
- `final-report`
