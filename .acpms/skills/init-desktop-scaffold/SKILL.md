---
name: init-desktop-scaffold
description: Create a desktop application baseline with a runnable shell, packaging-aware structure, and clear separation between app runtime layers.
---

# Init Desktop Scaffold

## Objective
Bootstrap a desktop app that is runnable and packaging-aware, while keeping the
initial scaffold simple enough to iterate on safely.

## When This Applies
- Project type is desktop
- ACPMS is creating an Electron, Tauri, Wails, or similar desktop baseline

## Inputs
- Project brief
- Requested stack or framework
- Target platforms, if known

## Workflow
1. Select the desktop stack based on explicit requirements.
2. Create the desktop runtime shell and frontend/backend split appropriate to
   the stack.
3. Add basic packaging/build configuration.
4. Create README, ignore files, and environment/config stubs.
5. Leave a runnable dev path and a clear package path.

## Required Baseline
- main/runtime entrypoint
- UI entrypoint
- build/package config
- README
- `.gitignore`

## Decision Rules
| Situation | Action |
|---|---|
| Stack explicitly specified | Follow it |
| Signing/notarization unavailable | Document it; do not pretend it is solved |
| Native integrations are not required yet | Stub structure, do not overbuild |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `created_files`
- `package_strategy`
- `verification_commands`

## Related Skills
- `init-project-bootstrap`
- `preview-artifact-desktop`
- `verify-test-build`
