---
name: verify-test-build
description: Run relevant verification commands and report pass, fail, or skipped with reasons.
---

# Verify Test Build

## Objective
Provide evidence that the implemented change works and does not break critical paths.

## Inputs
- Project scripts/tooling (`package.json`, `Cargo.toml`, Makefile, CI config).
- Task risk profile (feature/bug/refactor/docs/test).

## Workflow
1. Select smallest useful command set first, then widen only when needed.
2. Prefer project-native commands over ad-hoc commands.
3. Run verification after implementation is complete.
4. Capture command outputs and summarize failures by root cause.

## Command Selection Order
1. Targeted tests for changed area.
2. Lint/type checks (if configured).
3. Build command.
4. Broader test suite only when risk is high or targeted tests are unavailable.

## Decision Rules
| Situation | Action |
|---|---|
| Command unavailable in environment | Mark as `skipped` and explain prerequisite. |
| Command fails due to unrelated baseline issue | Mark as `failed_unrelated` and include evidence. |
| Time-expensive suite not necessary | Skip with explicit reason and residual risk. |

## Output Contract
Include a `Verification` section with one line per command:
- `command`: exact command.
- `status`: `pass` | `fail` | `skipped` | `failed_unrelated`.
- `notes`: short reason or key error.
