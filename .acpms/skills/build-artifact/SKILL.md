---
name: build-artifact
description: Produce deployment-ready artifacts and verify artifact paths for downstream deployment.
---

# Build Artifact

## Objective
Generate valid artifacts for deployment or distribution and confirm outputs are usable.

## Inputs
- Project type (`web`, `api`, `microservice`, `desktop`, `mobile`, `extension`).
- Existing build scripts/configuration in repository.

## Workflow
1. Detect canonical build command from existing scripts/config files.
2. Run build with production-safe mode where applicable.
3. Validate artifact output exists and is non-empty.
4. Record artifact path(s) and artifact type(s) for handoff.

## Decision Rules
| Situation | Action |
|---|---|
| Multiple build targets exist | Build only target relevant to task and deployment path. |
| Build succeeds but output path is missing | Treat as build failure and report path mismatch. |
| Build tooling missing | Mark blocked and provide install/setup requirement. |

## Guardrails
- Do not claim deployment readiness without validated artifact outputs.
- Do not silently continue after build failure.

## Output Contract
Include `Build Artifact Summary`:
- `build_status`: `success` or `failed`.
- `build_command`: command used.
- `artifact_paths`: list of produced paths.
- `artifact_notes`: format/type and any constraints.
