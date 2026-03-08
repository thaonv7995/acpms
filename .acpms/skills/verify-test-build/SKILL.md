---
name: verify-test-build
description: Run relevant verification commands and report pass, fail, or skipped with reasons.
---

# Verify Test Build

## Objective
Provide concrete evidence that the implemented change works and that critical
project paths still hold. Prefer the smallest meaningful verification set
first, then expand only if risk or project conventions demand it.

## When This Applies
- After `code-implement` or any other file-changing skill
- Before Git/MR handoff
- After deploy or preview auto-fix when ACPMS needs a real pass/fail signal

## Inputs
- Project scripts and tooling (`package.json`, `Cargo.toml`, Makefile, CI config)
- Task risk profile: docs-only, bugfix, feature, refactor, deploy-sensitive
- Existing baseline issues already known in the repo

## Workflow
1. Choose the lightest useful verification set.
2. Prefer project-native commands over ad-hoc shell checks.
3. Run targeted checks first, then broaden only if needed.
4. Capture root cause, not just exit status, when something fails.
5. Distinguish task-caused failures from pre-existing unrelated failures.

## Decision Rules
| Situation | Action |
|---|---|
| Targeted check exists for touched area | Run it before full-suite checks. |
| Command is unavailable in the environment | Mark it as skipped with the prerequisite. |
| Failure is clearly unrelated baseline debt | Mark as `failed_unrelated` and include evidence. |
| Large suite adds little value for this scope | Skip it and call out residual risk. |

## Output Contract
Emit a `Verification` summary with one line per command:
- `command`
- `status`: `pass` | `fail` | `skipped` | `failed_unrelated`
- `notes`: short reason or key error

## Related Skills
- `code-implement`
- `build-artifact`
- `gitlab-ci-verify`
- `final-report`
