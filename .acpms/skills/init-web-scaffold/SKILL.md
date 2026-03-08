---
name: init-web-scaffold
description: Type-specific scaffolding requirements for Web Application projects.
---

# Init Web Application Scaffold

## Objective
Define the scaffold requirements for a new web application project. Use the
project name and description from the init instruction. Follow required stack
constraints when they are explicit.

Do not hard-code one framework for all web apps. If the user does not specify a
stack, choose the one that best matches the actual web product being requested.

## When This Applies
- The init flow is creating a new web project
- The project type is `web` and ACPMS needs a runnable first scaffold
- A repo already exists but needs to be normalized into a web-app baseline

## Inputs
- Project name, description, and any required stack from the init instruction
- Existing repository state, if the project is not empty
- Product shape inferred from the brief:
  - landing page / marketing site
  - SPA / dashboard / internal tool
  - SSR / content-heavy / SEO-sensitive site
  - imported existing app
- Preview/deploy expectation, especially if ACPMS preview should run from Docker
- Supporting services such as API backend, database, cache, queue, or worker
- Expected deliverables such as `README.md`, `.env.example`, source entry points,
  and verification commands

## Workflow
1. Analyze the current repository structure or create a new scaffold.
2. Infer the web app shape from the brief before picking a framework.
3. Select the stack that best fits that shape:
   - simple landing page -> lightweight static or Vite-based scaffold
   - SPA / dashboard -> client-first framework such as React + TypeScript
   - SSR / SEO-heavy site -> SSR-capable framework such as Next.js or equivalent
   - imported existing repo -> preserve the current viable stack
4. Set up the development environment:
   - `package.json`
   - build tooling appropriate to the selected stack
   - TypeScript config when applicable
5. Create essential project files:
   - `README.md`
   - `.gitignore`
   - `.env.example`
6. Create initial source structure:
   - `src/`
   - `public/`
   - basic routing if the chosen framework supports it
   - layout and app structure appropriate to the selected web shape
7. Add container runtime files when the web app will be previewed/deployed
   through Docker or when helper services are part of the baseline:
   - `Dockerfile`
   - `docker-compose.yml` when the app or helper services need orchestration
   - `nginx.conf` or equivalent only when the runtime actually needs it
8. Add lightweight quality tooling only when it helps the baseline scaffold.
9. Verify the scaffold with the lightest useful command set.

## Decision Rules
| Situation | Action |
|---|---|
| Required stack is explicitly specified | Follow it; do not silently substitute a preferred framework. |
| User describes a product type but not a stack | Choose the stack from the product type and explain the reasoning. |
| App is a landing page or brochure site | Keep it light; do not introduce heavy SSR/fullstack tooling unless needed. |
| App is an interactive SPA or dashboard | Favor a component-based client stack with TypeScript. |
| App needs SSR or strong SEO/server routing | Favor an SSR-capable framework. |
| ACPMS preview is expected to run from Docker | Scaffold Docker runtime files during init, not later during deploy repair. |
| The web app depends on helper services or companion runtimes | Add `docker-compose.yml` that starts the app with those services from the first runnable scaffold. |
| Existing scaffold is mostly viable | Patch it instead of recreating the project from scratch. |
| Optional tooling slows bootstrap materially | Keep the scaffold lean and add extras only if required. |

## Output Contract
- Identify created or modified files
- Explain the selected stack, why it fits the app shape, and any important assumption
- Call out whether Docker runtime files were added and why
- Leave the project in a verifiable runnable state for the next init step

## Related Skills
- `init-project-bootstrap`
- `init-source-repository`
- `verify-test-build`
- `build-artifact`
