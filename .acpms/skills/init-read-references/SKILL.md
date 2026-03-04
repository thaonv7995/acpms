---
name: init-read-references
description: Read reference files from .acpms-refs/ before scaffolding (when user uploaded refs during project creation).
---

# Init Read References

## Objective
If reference files exist in `.acpms-refs/`, read them **before** scaffolding so the agent can replicate structure, patterns, or follow specs.

## When This Applies
- Init task (from-scratch project creation)
- Directory `.acpms-refs/` exists and is non-empty

## Workflow
1. **Check**: Does `.acpms-refs/` exist? If not, skip this skill.
2. **List**: Enumerate files in `.acpms-refs/` (including subdirectories if extracted from ZIP).
3. **Read**: Read relevant files (source code, configs, specs, mockups) to understand:
   - Project structure and naming
   - Tech stack and dependencies
   - Patterns to replicate
4. **Apply**: Use insights when executing init-project-bootstrap and init-source-repository.

## Decision Rules
| Situation | Action |
|-----------|--------|
| `.acpms-refs/` empty or missing | Skip, proceed to next skill. |
| ZIP/tar extracted with nested structure | Read key files (package.json, README, configs) first. |
| Images/PDF only | Extract requirements/design intent; mention in bootstrap summary. |

## Output Contract
- If refs were read: note in bootstrap summary which refs influenced the scaffold.
- If skipped: no output needed.
