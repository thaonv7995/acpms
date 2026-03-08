---
name: init-desktop-scaffold
description: Create a desktop application baseline with a runnable shell, packaging-aware structure, and clear separation between app runtime layers.
---

# Init Desktop Scaffold

## Objective
Bootstrap a desktop app that is runnable and packaging-aware, while keeping the
initial scaffold simple enough to iterate on safely.

Do not hard-code one desktop framework for all apps. If the user does not
explicitly specify a stack, choose the desktop stack that best fits the product
shape, native/runtime needs, and team constraints.

## When This Applies
- Project type is desktop
- ACPMS is creating an Electron, Tauri, Wails, or similar desktop baseline

## Inputs
- Project brief
- Requested stack or framework
- Target platforms, if known
- Product shape inferred from the brief:
  - lightweight desktop wrapper
  - native-heavy desktop app
  - JS/TS desktop app with complex shell integrations
  - imported existing desktop app
- Repo-shape clues:
  - standalone desktop repo
  - desktop app inside a monorepo

## Workflow
1. Decide repo shape from the brief or existing layout:
   - standalone desktop app repo
   - desktop app inside a monorepo
2. Select the desktop stack:
   - explicit stack requirement -> follow it
   - lightweight native-feeling app -> prefer Tauri or equivalent
   - JS/TS app with broad Node/native module ecosystem needs -> prefer Electron
   - Go-heavy desktop stack -> prefer Wails or equivalent
   - imported existing app -> preserve the current viable stack
3. Create the desktop runtime shell and frontend/backend split appropriate to
   the selected stack.
4. Add basic packaging/build configuration.
5. Create README, ignore files, and environment/config stubs.
6. Leave a runnable dev path and a clear package path.

## Required Baseline
- main/runtime entrypoint
- UI entrypoint
- build/package config
- README
- `.gitignore`
- dev run command
- package/build command path

## Decision Rules
| Situation | Action |
|---|---|
| Stack explicitly specified | Follow it |
| Brief implies the app lives with other apps/services in one repo | Preserve or create a monorepo-friendly layout; do not force standalone structure |
| Imported existing app already has a viable stack | Preserve it instead of re-platforming |
| Signing/notarization unavailable | Document it; do not pretend it is solved |
| Native integrations are not required yet | Stub structure, do not overbuild |
| ACPMS only needs artifact or app-run preview | Do not add Docker runtime files by default |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `repo_shape_selected`
- `created_files`
- `package_strategy`
- `verification_commands`
- `desktop_preview_strategy`
- `assumptions`

## Related Skills
- `init-project-bootstrap`
- `monorepo-service-selector`
- `preview-artifact-desktop`
- `verify-test-build`
