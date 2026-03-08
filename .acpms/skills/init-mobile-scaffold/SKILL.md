---
name: init-mobile-scaffold
description: Create a mobile application baseline with platform-aware structure, run instructions, and the minimum setup needed for simulator/device development.
---

# Init Mobile Scaffold

## Objective
Bootstrap a mobile app that can move quickly into real feature work without
hiding platform constraints around Android, iOS, signing, or simulator setup.

Do not hard-code one mobile framework for all apps. If the user does not
explicitly specify a stack, choose the mobile stack that best fits the product
shape and team/runtime constraints.

## When This Applies
- Project type is mobile
- ACPMS is creating a new mobile app baseline

## Inputs
- Project brief
- Requested stack or framework
- Target platform assumptions
- Product shape inferred from the brief:
  - cross-platform MVP
  - native-heavy cross-platform app
  - mobile-first product with strong platform polish
  - imported existing mobile app
- Repo-shape clues:
  - standalone mobile app repo
  - mobile app inside a monorepo

## Workflow
1. Decide repo shape from the brief or existing layout:
   - standalone mobile app repo
   - mobile app inside a monorepo
2. Choose the mobile stack:
   - explicit stack requirement -> follow it
   - cross-platform MVP -> prefer React Native + Expo or equivalent fast-iteration stack
   - native-heavy cross-platform app -> prefer bare React Native or equivalent
   - mobile-first product with strong platform polish -> prefer Flutter or the explicitly requested stack
   - imported existing app -> preserve the current viable stack
3. Create the core app entrypoint and app structure.
4. Add platform-aware config and environment stubs.
5. Add navigation baseline, screen structure, and shared UI/theme foundations.
6. Add README and developer run instructions for simulator/device workflows.
7. Leave the project in a state that can be built, bundled, or run in
   simulator/emulator without pretending unavailable signing/device steps work.

## Required Baseline
- app source entrypoint
- platform config
- navigation baseline
- screen/app structure
- README
- `.gitignore`
- build/run command path
- `.env.example` when the app expects runtime config

## Decision Rules
| Situation | Action |
|---|---|
| Framework is explicitly requested | Follow it unless it is impossible in the current environment |
| Cross-platform framework is unspecified | Choose the fastest stack that fits the requested app shape |
| Brief implies the mobile app lives with other apps/services in one repo | Preserve or create a monorepo-friendly layout; do not force standalone structure |
| Imported existing app already has a viable stack | Preserve it instead of re-platforming |
| iOS signing is unavailable | Document the limitation, do not fake installability |
| One platform is out of scope | Be explicit rather than pretending dual-platform parity |
| ACPMS preview path only needs a mobile artifact or dev bundle | Do not add Docker runtime files by default |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `repo_shape_selected`
- `created_files`
- `platform_support`
- `verification_commands`
- `mobile_preview_strategy`
- `assumptions`

## Related Skills
- `init-project-bootstrap`
- `monorepo-service-selector`
- `preview-artifact-mobile`
- `verify-test-build`
