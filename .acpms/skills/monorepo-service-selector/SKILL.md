---
name: monorepo-service-selector
description: Select the correct service, app, package, or workspace inside a monorepo so ACPMS builds, tests, deploys, and previews only the relevant target.
---

# Monorepo Service Selector

## Objective
Prevent ACPMS and the agent from running repo-wide commands blindly when the
task only affects one service, app, or package inside a monorepo.

## When This Applies
- The repository is a monorepo or multi-package workspace
- The task affects a specific app/service/package
- Preview, build, test, or deploy steps must target the correct leaf project

## Inputs
- Monorepo manifests and workspace config
- Task scope
- Changed files or target area

## Workflow
1. Detect the monorepo structure and workspace tooling.
2. Map changed files and task intent to the likely affected package(s).
3. Select the narrowest service/app set that satisfies the task.
4. Recommend scoped build/test/deploy commands.
5. Report when the task truly requires repo-wide verification.

## Decision Rules
| Situation | Action |
|---|---|
| One service/package is clearly affected | Use scoped commands |
| Shared package affects multiple downstream apps | Expand scope to the impacted set |
| Scope is unclear | Start narrow, explain assumptions |
| Changes are truly cross-cutting | Use broader verification intentionally |

## Output Contract
Emit:
- `monorepo_target_selection`
- `monorepo_selected_packages`
- `monorepo_command_scope`
- `monorepo_selection_reason`

## Related Skills
- `build-artifact`
- `verify-test-build`
- `task-scope-guard`

