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
shape, native/runtime depth, platform targets, and team constraints.

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
- Native integration depth inferred from the brief:
  - light shell only
  - moderate OS integration
  - deep platform/system integration
- Repo-shape clues:
  - standalone desktop repo
  - desktop app inside a monorepo

## Workflow
1. Decide repo shape from the brief or existing layout:
   - standalone desktop app repo
   - desktop app inside a monorepo
2. Select the desktop stack:
   - explicit stack requirement -> follow it
   - lightweight native-feeling cross-platform app -> prefer Tauri or equivalent
   - JS/TS app with broad Node/native module ecosystem needs -> prefer Electron
   - Go-heavy desktop stack -> prefer Wails or equivalent
   - macOS-first app with deep system integration -> prefer Swift/SwiftUI/AppKit
   - Windows-first app with deep system integration -> prefer .NET/WinUI/WPF or equivalent
   - Linux-first app with deep native desktop requirements -> prefer the native toolkit/runtime most appropriate to the distro target
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
| App is cross-platform and mostly UI/shell logic | Prefer a cross-platform desktop stack before reaching for a platform-native one |
| App needs moderate native integration but still targets multiple OSes | Prefer a cross-platform runtime with a credible native bridge story |
| App is platform-specific and must integrate deeply with the OS | Prefer the platform-native language/toolkit over Electron/Tauri/Wails |
| macOS app needs deep system hooks, privileged APIs, or native UX fidelity | Prefer Swift/SwiftUI/AppKit unless the user explicitly requires another stack |
| Windows app needs deep shell/system integration | Prefer a Windows-native stack such as .NET/WinUI/WPF unless the user explicitly requires another stack |
| Signing/notarization unavailable | Document it; do not pretend it is solved |
| Native integrations are not required yet | Stub structure, do not overbuild |
| ACPMS only needs artifact or app-run preview | Do not add Docker runtime files by default |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `stack_selection_reason`
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
