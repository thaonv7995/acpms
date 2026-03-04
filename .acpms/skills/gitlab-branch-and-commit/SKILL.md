---
name: gitlab-branch-and-commit
description: Execute safe Git/GitLab branch, staging, commit, and push workflow for task delivery.
---

# GitLab Branch And Commit

## Objective
Create a clean, traceable Git history for task delivery without leaking unrelated changes.

## Inputs
- Task context (title, type, scope).
- Current repo status and branch.
- Review policy (`require_review` behavior from workflow).

## Workflow
1. Inspect `git status --short` and identify only task-relevant files.
2. Ensure current branch is task-safe; if creating a branch, use `codex/<task-slug>` prefix.
3. Stage only intended files.
4. Commit with clear message (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`).
5. Push to remote branch.

## Never Stage or Commit
Never add or commit these paths (testing artifacts, internal tooling, config):
- `.playwright-cli/` — Playwright CLI output (screenshots, traces, YAML)
- `.acpms/` — ACPMS agent config, references, skills
- `node_modules/`, `dist/`, `target/`, `.env*`
- `*.log`, `tmp/`, `temp/`
- IDE/OS: `.idea/`, `.vscode/`, `.DS_Store`

Before staging, run `git status` and exclude any paths matching the above. Prefer `git add <file>` for specific files over `git add .`.

## Decision Rules
| Situation | Action |
|---|---|
| Review mode forbids commit/push | Do not commit or push; report ready-for-review state. |
| Unrelated modified files exist | Exclude from staging and mention in report. |
| Push rejected (non-fast-forward) | Pull/rebase safely or stop and report branch divergence. |

## Guardrails
- Never use `git add .` when unrelated changes are present.
- Never rewrite shared branch history unless explicitly requested.

## Output Contract
Include:
- `git_branch`
- `git_commit`
- `git_push`: `success` | `skipped` | `failed`
- `git_push_reason` when skipped/failed
