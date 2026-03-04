---
name: db-migration-safety
description: Execute schema migration changes safely with backward compatibility, validation, and rollback awareness.
---

# DB Migration Safety

## Objective
Apply schema changes without breaking running services or data integrity.

## Inputs
- Proposed migration scripts.
- Current schema assumptions and dependent code paths.
- Rollback and deployment constraints.

## Workflow
1. Classify migration type (additive/change/drop/backfill).
2. Prefer backward-compatible, additive migrations first.
3. Update application code for dual-read/write when necessary.
4. Validate migration on test/staging path.
5. Document rollback and data safety notes.

## Decision Rules
| Migration Pattern | Safe Default |
|---|---|
| Add column/table/index | Usually safe additive step. |
| Rename/drop column | Use multi-step migration with compatibility bridge. |
| Type change | Use shadow column/backfill/switch strategy. |
| Large backfill | Run chunked job, not blocking request path. |

## Guardrails
- Do not bundle irreversible destructive steps with unverified deploy.
- Keep migrations idempotent when possible.

## Output Contract
Include:
- `migration_risk`: `low` | `medium` | `high`
- `migration_strategy`
- `compatibility_notes`
- `rollback_plan`
