---
name: gitlab-ci-verify
description: Verify GitLab CI pipeline status for pushed changes and report gate outcome.
---

# GitLab CI Verify

## Objective
Validate that pushed changes satisfy CI gates before marking delivery complete.

## Inputs
- Branch/ref of pushed commit.
- Pipeline URL or project CI context.

## Workflow
1. Locate latest pipeline for commit/branch.
2. Read stage/job statuses.
3. Classify result as pass/fail/pending.
4. Report blocking jobs and failure excerpts.

## Decision Rules
| Situation | Action |
|---|---|
| Pipeline pending | Mark `ci_pending` and continue with clear status note. |
| Required job failed | Mark `ci_failed` and include blocking job names. |
| Pipeline passed | Mark `ci_passed`. |

## Output Contract
Include:
- `ci_status`: `ci_passed` | `ci_failed` | `ci_pending` | `ci_skipped`
- `ci_pipeline_url`
- `ci_blocking_jobs` (if any)
