---
name: db-migration-safety
description: Plan and execute schema changes safely with backward compatibility, staged rollout thinking, validation, and rollback awareness.
---

# DB Migration Safety

## Objective
Make schema or data-shape changes without breaking running application code,
corrupting data, or creating an irreversible deploy path.

## When This Applies
- The task changes database schema
- The task adds, renames, or removes columns/tables/indexes
- The task introduces data backfill or migration scripts
- Application and migration changes will be deployed together

## Inputs
- Proposed migration files or schema diff
- Current application read/write behavior
- Database engine and migration tooling
- Deployment/rollback constraints

## Workflow
1. Classify the migration pattern:
   - additive
   - rename/drop
   - type change
   - backfill
   - constraint/index change
2. Prefer additive, backward-compatible steps first.
3. If destructive change is needed, split into multiple deploy-safe stages.
4. Ensure application code can tolerate the intermediate schema state.
5. Validate migration behavior in the safest available environment.
6. Record rollback and compatibility notes before marking the task ready.

## Decision Rules
| Migration Pattern | Safe Default |
|---|---|
| Add column/table/index | Usually safe additive step |
| Rename/drop column | Multi-step migration with compatibility bridge |
| Type change | Shadow column, backfill, switch-over strategy |
| Large backfill | Chunked job, not request-path blocking |
| New constraint | Add only after data already satisfies it |

## Guardrails
- Never bundle irreversible destructive changes with an unverified deploy
- Never assume application code and schema change can land simultaneously if old
  nodes may still run
- Prefer idempotent migrations when the tooling supports it

## Output Contract
Emit:
- `migration_risk`: `low` | `medium` | `high`
- `migration_strategy`
- `compatibility_notes`
- `rollback_plan`

## Related Skills
- `code-implement`
- `verify-test-build`
- `rollback-deploy`
