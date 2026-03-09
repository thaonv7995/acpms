---
name: init-source-repository
description: Initialize the source repository on GitLab or GitHub, push the first commit, and write REPO_URL to the init output contract.
---

# Init Source Repository

## Objective
Create or connect the remote source repository, push the initial commit, and
write `repo_url` into `.acpms/init-output.json` so ACPMS can persist it
reliably.

## When This Applies
- A new project needs its first remote repository
- ACPMS init flow requires machine-readable `repo_url` output after push
- The remote may already exist and only needs to be connected and recorded

## Inputs
- Project name and slug
- `GITLAB_PAT` for GitLab or GitHub auth
- `GITLAB_URL` to determine provider and base URL
- Requested visibility

## Workflow
1. Create or locate the remote repository on the correct provider.
2. Initialize git locally if needed and ensure there is a valid first commit.
3. Add the remote URL and push the default branch.
4. Only after push succeeds, write `.acpms/init-output.json`.
5. If the remote already exists, reuse it and still persist the contract file.

## Decision Rules
| Situation | Action |
|---|---|
| Push succeeds | Write `.acpms/init-output.json` immediately after push. |
| Push fails | Stop, report the error, and do not write the contract file. |
| Repo already exists | Reuse the existing URL and still write the contract file. |

## Output Contract
Write:

```json
{"repo_url":"https://example-host/org/project-name"}
```

Also emit:
- `init_status`
- `repo_url`

## Related Skills
- `init-project-bootstrap`
- `gitlab-branch-and-commit`
- `gitlab-merge-request`
- `final-report`
