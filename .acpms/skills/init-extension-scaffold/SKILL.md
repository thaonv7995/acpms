---
name: init-extension-scaffold
description: Create a browser extension baseline with manifest, background runtime, UI surface, and build output suitable for QA and later store packaging.
---

# Init Extension Scaffold

## Objective
Bootstrap a browser extension that is runnable as an unpacked extension and has
the minimum correct structure for permissions, background logic, and UI.

## When This Applies
- Project type is browser extension
- ACPMS is creating a new extension from scratch

## Inputs
- Project brief
- Browser target assumptions
- Required UI surfaces, if specified

## Workflow
1. Choose the extension toolchain and manifest strategy.
2. Create the manifest with least-privilege defaults.
3. Create background runtime and the required UI surfaces.
4. Add build tooling, README, and ignore files.
5. Ensure the extension can be built and loaded unpacked.

## Required Baseline
- manifest
- background/service worker
- popup or equivalent UI when needed
- build config
- README

## Decision Rules
| Situation | Action |
|---|---|
| Permissions are unclear | Start minimal |
| Multi-browser support is not required yet | Optimize for the primary browser first |
| Framework is unspecified | Choose the lightest maintainable default |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `created_files`
- `manifest_version`
- `verification_commands`

## Related Skills
- `init-project-bootstrap`
- `preview-artifact-extension`
- `verify-test-build`
