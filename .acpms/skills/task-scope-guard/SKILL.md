---
name: task-scope-guard
description: Keep the agent inside the real task scope and stop unrelated deploy, preview, refactor, or cleanup work from leaking into small or docs-only tasks.
---

# Task Scope Guard

## Objective
Prevent unnecessary work by making the agent distinguish between the requested
task and adjacent but out-of-scope improvements, deploys, or cleanup.

## When This Applies
- Task is small, docs-only, or narrowly scoped
- The agent is tempted to run broad verification or deploy flows
- A failure outside the touched scope appears during verification

## Inputs
- Task title and description
- Touched files
- Proposed next actions such as deploy, preview, refactor, cleanup, or broad
  test runs

## Workflow
1. Restate the narrow scope of the task.
2. Identify which checks and actions are required versus optional.
3. Block or demote actions that do not meaningfully support the task outcome.
4. Allow broader actions only when the task explicitly requires them or when
   they are necessary to validate the requested change.

## Decision Rules
| Situation | Action |
|---|---|
| Docs-only task | Avoid deploy and preview flows unless explicitly requested |
| Small scoped change | Prefer targeted verification over broad repo-wide work |
| Pre-existing unrelated failure appears | Report it once; do not let it explode scope unless it blocks the requested outcome |
| Task explicitly asks for deploy or preview | Permit those flows even if code change is small |

## Output Contract
Emit:
- `scope_guard_status`: `in_scope` | `out_of_scope_blocked` | `expanded_with_reason`
- `required_actions`
- `skipped_actions`
- `scope_guard_reason`

## Related Skills
- `code-implement`
- `test-failure-triage`
- `verify-test-build`
- `release-note-and-delivery-summary`
