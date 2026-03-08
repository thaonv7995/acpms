---
name: code-implement
description: Implement task-scoped changes with minimal blast radius and production-safe behavior.
---

# Code Implement

## Objective
Implement the requested change with the smallest correct edit set. Optimize for
task scope, predictable review, and production safety. This skill owns the
actual file changes; it should not silently expand into deploy, migration, or
repo-management work unless another skill explicitly requires that step.

## When This Applies
- The task is past preflight and ready for real file changes
- ACPMS needs implementation in source files, config files, templates, or
  task-scoped docs
- The request is not only planning, diagnosis, or reporting

## Inputs
- Task title, description, and acceptance criteria
- Current repository state in the assigned worktree
- Existing architecture, conventions, and verification hooks
- Any references already collected by preflight or requirement skills

## Workflow
1. Restate scope in one to three sentences and keep the change set narrow.
2. Read only the files needed to implement the task correctly.
3. Make small, coherent edits instead of broad speculative refactors.
4. Re-read edited files for regressions, dead code, and side effects.
5. Remove temporary debug code, experiment scaffolding, or abandoned branches.
6. Hand off to verification once the change set is stable.

## Decision Rules
| Situation | Action |
|---|---|
| Requirement is ambiguous | Make the safest assumption and report it. |
| Broad refactor is not required for acceptance | Keep scope narrow and document debt instead. |
| Unrelated issue is discovered | Do not fix opportunistically unless it blocks the task. |
| Docs-only or metadata-only task | Keep implementation lightweight and avoid unnecessary code churn. |

## Output Contract
Final reporting should include:
- `Changed Files`: exact files touched
- `Scope Notes`: what was intentionally not changed
- `Assumptions`: only when a meaningful ambiguity existed

## Related Skills
- `task-preflight-check`
- `verify-test-build`
- `review-handoff`
- `final-report`
