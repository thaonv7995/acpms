---
name: test-failure-triage
description: Classify build, lint, and test failures into task-caused, pre-existing, flaky, environment, or unrelated failures so ACPMS can react appropriately.
---

# Test Failure Triage

## Objective
Avoid treating every failing build or test as equally blocking by determining
whether the failure was introduced by the task, existed beforehand, or is
caused by environment or flake.

## When This Applies
- `npm run lint`, `npm test`, `cargo test`, or build verification fails
- ACPMS needs to decide whether to block, continue with warning, or retry
- The task scope is small and a large unrelated failure appears

## Inputs
- Failing command
- Failure output
- Task scope and touched files
- Prior known baseline failures, if available

## Workflow
1. Identify the failing command and failing files/tests.
2. Compare the failure with the task’s touched files and scope.
3. Classify the failure:
   - task-caused
   - pre-existing
   - flaky/transient
   - environment/config
   - unrelated but still blocking
4. Decide whether to fix now, report and continue, or stop.
5. Produce a concise classification for the final report and retry logic.

## Decision Rules
| Situation | Action |
|---|---|
| Failure is in changed code or directly caused by the task | Fix or block |
| Failure is clearly pre-existing and outside scope | Report once and continue if task output is still trustworthy |
| Failure is flaky or transient | Retry or mark unstable |
| Failure is due to missing env/deps | Classify as environment/config issue |

## Output Contract
Emit:
- `test_failure_classification`
- `test_failure_scope_relation`
- `test_failure_action`
- `test_failure_summary`

## Related Skills
- `verify-test-build`
- `retry-triage-and-recovery`
- `task-scope-guard`

