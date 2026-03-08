---
name: init-extension-scaffold
description: Create a browser extension baseline with manifest, background runtime, UI surface, and build output suitable for QA and later store packaging.
---

# Init Extension Scaffold

## Objective
Bootstrap a browser extension that is runnable as an unpacked extension and has
the minimum correct structure for permissions, background logic, and UI.

Do not hard-code one extension toolchain for all projects. If the user does not
explicitly specify a stack, choose the extension stack that best fits the
required browser targets, UI surface, and maintainability needs.

## When This Applies
- Project type is browser extension
- ACPMS is creating a new extension from scratch

## Inputs
- Project brief
- Browser target assumptions
- Required UI surfaces, if specified
- Product shape inferred from the brief:
  - popup-only extension
  - content-script-heavy extension
  - background/service-worker-heavy extension
  - multi-browser extension
  - imported existing extension
- Repo-shape clues:
  - standalone extension repo
  - extension inside a monorepo

## Workflow
1. Decide repo shape from the brief or existing layout:
   - standalone extension repo
   - extension inside a monorepo
2. Choose the extension toolchain and manifest strategy:
   - explicit stack requirement -> follow it
   - lightweight popup/content extension -> prefer the lightest maintainable stack
   - complex extension UI or state model -> choose a framework/toolchain that matches the UI needs
   - imported existing extension -> preserve the current viable stack
3. Create the manifest with least-privilege defaults.
4. Create background runtime and the required UI surfaces.
5. Add build tooling, README, and ignore files.
6. Ensure the extension can be built and loaded unpacked.

## Required Baseline
- manifest
- background/service worker
- popup or equivalent UI when needed
- build config
- README
- `.gitignore`
- extension load/build command path

## Decision Rules
| Situation | Action |
|---|---|
| Permissions are unclear | Start minimal |
| Brief implies the extension lives inside a larger repo | Preserve or create a monorepo-friendly layout; do not force standalone structure |
| Multi-browser support is not required yet | Optimize for the primary browser first |
| Framework is unspecified | Choose the lightest maintainable stack that fits the extension shape |
| Imported existing extension already has a viable stack | Preserve it instead of re-platforming |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `repo_shape_selected`
- `created_files`
- `manifest_version`
- `verification_commands`
- `extension_preview_strategy`
- `assumptions`

## Related Skills
- `init-project-bootstrap`
- `monorepo-service-selector`
- `preview-artifact-extension`
- `verify-test-build`
