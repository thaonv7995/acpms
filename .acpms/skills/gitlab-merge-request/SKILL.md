---
name: gitlab-merge-request
description: Create or update a Merge Request (GitLab) or Pull Request (GitHub) with task summary, verification evidence, and deployment notes.
---

# Merge Request / Pull Request

## Objective
Produce a review-ready MR (GitLab) or PR (GitHub) that captures scope, evidence, and rollout risk. Use the appropriate API based on remote host (GITLAB_URL): GitLab → MR API, GitHub → PR API.

## Inputs
- Branch with pushed commit(s).
- Task summary and verification results.
- Deployment outcome (success/failed/skipped).
- GITLAB_URL: Base URL (e.g. https://gitlab.com or https://github.com) — determines provider.
- GITLAB_PAT: PAT for GitLab or GitHub.

## Workflow
1. Detect provider from `git remote get-url origin` or GITLAB_URL: github.com → GitHub PR, else → GitLab MR.
2. Create MR/PR from task branch to target branch (GitLab API or GitHub API / `gh pr create`).
3. Populate title/body with concise summary.
4. Include verification results and deployment notes.
5. Add risk notes and rollback guidance when relevant.

## MR/PR Template (Minimum)
- `Summary`
- `Changes`
- `Verification`
- `Deployment`
- `Risks / Rollback`

## Decision Rules
| Situation | Action |
|---|---|
| MR/PR already exists | Update existing description/comments instead of creating duplicate. |
| Pipeline is pending | Mark as waiting for CI and include current pipeline URL/state. |
| Deployment skipped by policy | State reason explicitly in `Deployment` section. |
| GitHub repo | Use `gh pr create` or GitHub REST API (POST /repos/{owner}/{repo}/pulls). |
| GitLab repo | Use GitLab API (POST /projects/{id}/merge_requests). |

## Output Contract

### Option A — File Contract (Recommended, more reliable)
Ghi file `.acpms/mr-output.json` trước khi push/hoàn thành task để hệ thống extract MR/PR title/description chính xác:

```json
{"mr_title": "feat: Add user auth flow", "mr_description": "## Summary\n..."}
```

- Tạo thư mục `.acpms/` nếu chưa có: `mkdir -p .acpms`
- Ghi file với JSON hợp lệ (escape dấu `"` trong nội dung). Ví dụ đơn giản:
  `node -e "require('fs').writeFileSync('.acpms/mr-output.json', JSON.stringify({mr_title:'feat: X', mr_description:'## Summary\\n...'}))"`
- Hệ thống đọc file này, lưu vào metadata, rồi xóa file (giống init-output.json cho repo_url).

### Option B — Log output (fallback)
Include in final report (for MR/PR creation):
- `MR_TITLE`: Short title for the merge/pull request (e.g. "feat: Add user auth flow")
- `MR_DESCRIPTION`: Markdown body with Summary, Changes, Verification, Deployment, Risks

Also include when MR/PR exists:
- `mr_action`: `created` | `updated` | `skipped`
- `mr_iid` (GitLab) or `pr_number` (GitHub)
- `mr_url` or `pr_url`
- `mr_status_note`
