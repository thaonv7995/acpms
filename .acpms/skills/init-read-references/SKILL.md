---
name: init-read-references
description: Read `.acpms-refs/` before scaffolding so uploaded references influence the generated project instead of being ignored until after bootstrap.
---

# Init Read References

## Objective
Use uploaded reference material early in init so the scaffold reflects real
constraints, examples, designs, or patterns from the start.

## When This Applies
- `.acpms-refs/` exists and contains files
- The task is an init/bootstrap flow
- The user uploaded code, specs, mockups, or sample projects as references

## Inputs
- Contents of `.acpms-refs/`
- Project type and init goal

## Workflow
1. Check whether `.acpms-refs/` exists and is non-empty.
2. Identify the most informative files first:
   - package manifests
   - configs
   - README/specs
   - source structure
3. Read only the relevant references needed to steer the scaffold.
4. Carry the extracted decisions into bootstrap and scaffold skills.
5. Mention in the bootstrap summary which references materially influenced the
   output.

## Decision Rules
| Situation | Action |
|---|---|
| Reference directory missing or empty | Skip cleanly |
| Archive or nested project present | Read key manifests/configs first |
| Only images or design docs exist | Extract design/intent, not code structure |

## Output Contract
Emit:
- `reference_read_status`: `used` | `skipped`
- `reference_summary`
- `reference_influence`

## Related Skills
- `init-project-bootstrap`
- `init-web-scaffold`
- `init-api-scaffold`
