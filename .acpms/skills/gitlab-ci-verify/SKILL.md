---
name: gitlab-ci-verify
description: Check the GitLab pipeline for the pushed branch or commit and classify whether delivery is blocked, pending, passed, or intentionally skipped.
---

# GitLab CI Verify

## Objective
Report CI gate status accurately so ACPMS does not imply that pushed code is
fully verified when the GitLab pipeline is still pending or already failed.

## When This Applies
- Changes were pushed to GitLab
- The task or delivery flow depends on CI visibility
- A merge or review handoff should mention real pipeline state

## Inputs
- Branch name or commit SHA
- GitLab project context
- Latest pipeline URL when available

## Workflow
1. Locate the latest pipeline for the pushed branch or commit.
2. Read overall pipeline state and critical job states.
3. Classify as passed, failed, pending, or skipped.
4. Surface blocking jobs when the pipeline is red.

## Decision Rules
| Situation | Action |
|---|---|
| No CI configured for this repo | Mark `ci_skipped` |
| Pipeline still running | Mark `ci_pending` |
| Required job failed | Mark `ci_failed` and list blockers |
| Pipeline completed successfully | Mark `ci_passed` |

## Output Contract
Emit:
- `ci_status`: `ci_passed` | `ci_failed` | `ci_pending` | `ci_skipped`
- `ci_pipeline_url`
- `ci_blocking_jobs`

## Related Skills
- `gitlab-merge-request`
- `review-handoff`
- `release-note-and-delivery-summary`
