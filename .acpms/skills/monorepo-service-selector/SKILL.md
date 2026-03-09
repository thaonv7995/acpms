---
name: monorepo-service-selector
description: Select the correct service, app, package, or workspace inside a monorepo so ACPMS builds, tests, deploys, and previews only the relevant target.
---

# Monorepo Service Selector

## Objective
Prevent ACPMS and the agent from running repo-wide commands blindly when the
task only affects one service, app, or package inside a monorepo.

This skill is only about repo shape and targeting. It does not mean the project
type is automatically `microservice`; it can apply to web, API, microservice,
worker, or mixed repos.

## When This Applies
- The repository is a monorepo or multi-package workspace
- The task affects a specific app/service/package
- Preview, build, test, or deploy steps must target the correct leaf project
- The brief explicitly asks for multiple services/apps/packages in one repo

## Inputs
- Monorepo manifests and workspace config
- Project brief or init requirement when the repo has not been created yet
- Task scope
- Changed files or target area

## Workflow
1. Decide whether the repo shape is truly monorepo:
   - explicit multi-service or multi-app brief
   - workspace manifests
   - existing package/service directories
2. Detect the monorepo structure and workspace tooling.
3. Map changed files and task intent to the likely affected package(s).
4. Select the narrowest service/app set that satisfies the task.
5. Recommend scoped build/test/deploy commands.
6. Report when the task truly requires repo-wide verification.

## Decision Rules
| Situation | Action |
|---|---|
| Brief describes one standalone service/app | Do not force monorepo layout or monorepo tooling |
| Brief explicitly calls for multiple services/apps/packages in one repo | Treat it as monorepo from init |
| One service/package is clearly affected | Use scoped commands |
| Shared package affects multiple downstream apps | Expand scope to the impacted set |
| Scope is unclear | Start narrow, explain assumptions |
| Changes are truly cross-cutting | Use broader verification intentionally |

## Output Contract
Emit:
- `repo_shape`: `standalone` | `monorepo`
- `monorepo_target_selection`
- `monorepo_selected_packages`
- `monorepo_command_scope`
- `monorepo_selection_reason`

## Related Skills
- `build-artifact`
- `verify-test-build`
- `task-scope-guard`
