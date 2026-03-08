---
name: init-web-scaffold
description: Type-specific scaffolding requirements for Web Application projects.
---

# Init Web Application Scaffold

## Objective
Define the scaffold requirements for a new web application project. Use the
project name and description from the init instruction. Follow required stack
constraints when they are explicit.

## When This Applies
- The init flow is creating a new web project
- The project type is `web` and ACPMS needs a runnable first scaffold
- A repo already exists but needs to be normalized into a web-app baseline

## Inputs
- Project name, description, and any required stack from the init instruction
- Existing repository state, if the project is not empty
- Expected deliverables such as `README.md`, `.env.example`, source entry points,
  and verification commands

## Workflow
1. Analyze the current repository structure or create a new scaffold.
2. Set up the development environment:
   - `package.json`
   - build tooling (Vite, Next.js, or similar)
   - TypeScript config when applicable
3. Create essential project files:
   - `README.md`
   - `.gitignore`
   - `.env.example`
4. Create initial source structure:
   - `src/`
   - `public/`
   - basic routing if the chosen framework supports it
5. Add lightweight quality tooling only when it helps the baseline scaffold.
6. Verify the scaffold with the lightest useful command set.

## Decision Rules
| Situation | Action |
|---|---|
| Required stack is explicitly specified | Follow it; do not silently substitute a preferred framework. |
| Existing scaffold is mostly viable | Patch it instead of recreating the project from scratch. |
| Optional tooling slows bootstrap materially | Keep the scaffold lean and add extras only if required. |

## Output Contract
- Identify created or modified files
- Explain the selected stack and any important assumption
- Leave the project in a verifiable runnable state for the next init step

## Related Skills
- `init-project-bootstrap`
- `init-source-repository`
- `verify-test-build`
- `build-artifact`
