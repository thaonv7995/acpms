---
name: code-implement
description: Implement task-scoped changes with minimal blast radius and production-safe behavior.
---

# Code Implement

## Objective
Ship only the code required by the task while preserving existing behavior outside scope.

## Inputs
- Task title and description.
- Project conventions and architecture.
- Current repository state in the assigned worktree.

## Workflow
1. Restate scope in 1-3 sentences and list acceptance criteria.
2. Identify minimal files required for the task.
3. Implement changes with small, coherent edits.
4. Re-read edited files for regressions and side effects.
5. Remove temporary debug code and dead paths introduced during implementation.

## Decision Rules
| Situation | Action |
|---|---|
| Requirement is ambiguous | Make the safest assumption and state it in the final report. |
| Change requires broad refactor | Stop broad refactor, keep scoped fix, document technical debt. |
| Unrelated failing area is discovered | Do not fix opportunistically unless it blocks task completion. |

## Guardrails
- Do not modify unrelated modules.
- Do not rename/move files unless required by task acceptance criteria.
- Honor review mode: if workflow says no commit/push, do not commit/push.

## Output Contract
In final report include:
- `Changed Files`: explicit list of modified files.
- `Scope Notes`: what was intentionally not changed.
- `Assumptions`: only if ambiguity existed.
