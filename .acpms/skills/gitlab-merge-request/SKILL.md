---
name: gitlab-merge-request
description: Create or update a Merge Request (GitLab) or Pull Request (GitHub) with task summary, verification evidence, and deployment notes.
---

# Merge Request / Pull Request

## Objective
Produce a review-ready MR or PR that captures scope, evidence, and rollout
risk. Use the correct provider flow based on the repository host.

## When This Applies
- The branch has already been pushed and is ready for review
- ACPMS needs a provider-level review object instead of only a local commit
- The task requires review handoff or updated MR/PR metadata

## Inputs
- Branch with pushed commit(s)
- Task summary and verification results
- Deployment outcome (success, failed, skipped)
- `GITLAB_URL` and `GITLAB_PAT`
- Existing MR/PR URL or ID if one already exists

## Workflow
1. Detect the provider from `git remote get-url origin` or `GITLAB_URL`.
2. Create or update the MR/PR from the task branch to the target branch.
3. Populate title and body with a concise task summary.
4. Include verification results and deployment notes.
5. Avoid duplicates; update the existing review object when appropriate.

## Decision Rules
| Situation | Action |
|---|---|
| MR/PR already exists | Update it instead of creating a duplicate. |
| Pipeline is pending | Mention that state and include current provider URL/state if available. |
| Deployment was skipped by policy | State the reason explicitly in the body. |
| Provider is GitHub | Use GitHub-native PR flow. |
| Provider is GitLab | Use GitLab-native MR flow. |

## Output Contract
Preferred file contract:

```json
{"mr_title":"feat: Example change","mr_description":"## Summary\\n..."}
```

Fallback fields:
- `MR_TITLE`
- `MR_DESCRIPTION`
- `mr_action`
- `mr_url` or `pr_url`
- `mr_status_note`

## Related Skills
- `gitlab-branch-and-commit`
- `verify-test-build`
- `review-handoff`
- `update-deployment-metadata`
