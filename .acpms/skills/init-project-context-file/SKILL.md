---
name: init-project-context-file
description: Generate PROJECT_CONTEXT.md with architecture overview and development guidelines for AI agents.
---

# Init Project Context File

## Objective
Create `PROJECT_CONTEXT.md` (agent-agnostic) to help AI assistants understand the project when working on it later. This file provides architecture overview, development guidelines, and key commands.

## When This Applies
- Init task (from-scratch project creation)
- After init-project-bootstrap has scaffolded the project

## Workflow
1. **Create** `PROJECT_CONTEXT.md` in the project root.
2. **Include** content appropriate for the project type:
   - **Web**: Project architecture overview, development guidelines, key commands and workflows
   - **API**: API architecture overview, endpoint documentation pattern, development guidelines
   - **Mobile**: Project architecture overview, platform-specific build instructions, development guidelines
   - **Extension**: Extension architecture overview, permission justifications, development and testing guidelines
   - **Desktop**: Architecture overview (main vs renderer process), security considerations, build and distribution guidelines
   - **Microservice**: Service architecture overview, API documentation, deployment guidelines, observability setup

## Decision Rules
| Situation | Action |
|-----------|--------|
| Project type known | Use type-specific content structure above. |
| Project type unclear | Use generic structure (architecture, guidelines, key commands). |
| Minimal scaffold | Keep brief; expand as project grows. |

## Output Contract
- File must exist at project root: `PROJECT_CONTEXT.md`
- Content should be concise but actionable for future AI agents.
- Do not duplicate README.md; focus on what AI needs (structure, patterns, commands).
