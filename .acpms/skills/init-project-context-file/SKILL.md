---
name: init-project-context-file
description: Create PROJECT_CONTEXT.md so future agents can understand the repo structure, commands, architecture, and important constraints without re-deriving everything from scratch.
---

# Init Project Context File

## Objective
Create `PROJECT_CONTEXT.md` in the project root as a compact operating manual for
future AI agents working in the repository.

## When This Applies
- A new project scaffold was created
- The repo is being initialized from scratch
- The repo has enough structure that a future agent would benefit from a
  summary of architecture, commands, and conventions

## Inputs
- Project type
- Actual project structure and entry points
- Main commands for install, dev, build, test, and deploy
- Important architectural or environment constraints

## Workflow
1. Inspect the repository structure and identify real entry points.
2. Gather the main developer commands that actually work.
3. Summarize architecture, key folders, and conventions.
4. Write `PROJECT_CONTEXT.md` in concise, agent-oriented language.
5. Keep it complementary to `README.md`, not a duplicate.

## Decision Rules
| Situation | Action |
|---|---|
| Project type is clear | Use type-specific structure |
| Project type is mixed or unclear | Use a generic architecture/commands/conventions layout |
| Project is still very minimal | Keep the context file short and factual |

## Output Contract
Produce `PROJECT_CONTEXT.md` containing at least:
- project purpose
- key structure
- main commands
- architecture notes
- important constraints

## Related Skills
- `init-project-bootstrap`
- `init-web-scaffold`
- `init-api-scaffold`
- `init-source-repository`
