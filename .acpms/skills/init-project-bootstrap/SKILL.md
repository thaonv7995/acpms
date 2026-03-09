---
name: init-project-bootstrap
description: Execute init-task bootstrap workflow for new projects with safe defaults and reproducible setup.
---

# Init Project Bootstrap

## Objective
Create a runnable baseline project from init requirements without over-scaffolding,
unsafe defaults, or optional tooling that slows the first usable state.

The bootstrap policy should stay flexible. If the user does not explicitly name
a framework or stack, choose the stack that best matches the product shape
rather than forcing a single default such as Vite or Next.js for every web app.

Treat `project type` and `repo shape` as separate decisions. For example, a
microservice can live in a single-service repo or inside a monorepo with
multiple services. Do not collapse those into one assumption.

## When This Applies
- ACPMS is handling a from-scratch project initialization
- A project brief exists but the repo still needs its first runnable baseline
- Type-specific scaffold skills need a shared bootstrap policy first

## Inputs
- Init task metadata and selected stack
- Existing repository state
- Required project settings such as visibility, naming, and base structure
- Preview/deploy expectations, especially when ACPMS preview is enabled
- Supporting runtime needs such as database, cache, queue, search, or worker
  processes
- Repo shape clues from the brief:
  - one service in one repo
  - multiple services/packages/apps in one repo
  - imported repo with existing workspace layout

## Workflow
1. Parse the init scope and any explicit stack requirements.
2. Infer the product shape before selecting a stack:
   - landing page / marketing site
   - SPA / dashboard / internal tool
   - SSR / content-heavy / SEO-sensitive site
   - realtime or app-like web client
   - single microservice repo
   - multi-service monorepo
   - imported existing project that should preserve its current stack
3. Infer repo shape separately from project type:
   - standalone repo
   - monorepo / multi-package / multi-service repo
4. Choose the simplest bootstrap path that satisfies the inferred app shape,
   repo shape, and any explicit requirements.
5. Decide whether container files must exist from init:
   - if ACPMS preview/deploy is expected to run from Docker
   - if the app depends on supporting services such as DB/cache/queue
   - if local multi-service startup would otherwise be ambiguous
6. Generate the minimum runnable skeleton.
7. Install dependencies and run baseline validation.
8. Produce a concise bootstrap summary for downstream init steps.

## Decision Rules
| Situation | Action |
|---|---|
| Stack is explicitly specified | Follow it exactly unless it is impossible, then report the constraint clearly. |
| Stack selection is incomplete | Choose the lightest stack that fits the app shape and report the assumption. |
| Imported repo already has a viable stack | Preserve and normalize that stack instead of re-platforming it. |
| Brief implies multiple services/apps/packages in one repo | Treat it as a monorepo-style layout and plan scoped service/app directories from init. |
| Brief describes one bounded service with no repo-shape clue | Treat it as a single-service repo first; do not invent a monorepo. |
| The app is a simple landing page or static marketing site | Prefer a lightweight static or Vite-based scaffold; avoid heavy SSR frameworks unless requested. |
| The app is a dashboard or interactive SPA | Prefer a client-first stack such as React + TypeScript, unless the brief strongly suggests another framework. |
| The app requires SSR, server rendering, or strong SEO needs | Prefer an SSR-capable framework such as Next.js or equivalent. |
| ACPMS preview/deploy expects Docker-based runtime | Include `Dockerfile` and, when startup orchestration is needed, `docker-compose.yml` during init. |
| App needs supporting services such as DB/cache/queue/worker | Include `docker-compose.yml` that wires the app and helper services together from init. |
| Bootstrap command fails | Stop, capture root cause, and provide recovery guidance. |
| Optional integrations are unavailable | Continue with the core scaffold and mark optional setup pending. |

## Output Contract
Emit:
- `init_status`
- `stack_selected`
- `stack_selection_reason`
- `repo_shape_selected`
- `containerization_plan`
- `bootstrap_commands`
- `bootstrap_validation`
- `bootstrap_assumptions`

## Related Skills
- `task-preflight-check`
- `init-web-scaffold`
- `init-source-repository`
- `verify-test-build`
