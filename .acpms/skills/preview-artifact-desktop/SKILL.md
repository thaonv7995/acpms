---
name: preview-artifact-desktop
description: Produce a QA-usable desktop build artifact for task preview, aligned with the project’s real packaging flow and output paths.
---

# Preview Artifact Desktop

## Objective
Generate a downloadable desktop artifact that QA can actually run or install,
without confusing artifact preview with live URL preview.

## When This Applies
- Project type is desktop
- Task preview should be delivered as downloadable artifacts

## Inputs
- Desktop packaging stack
- Actual package/build command
- Real output directory

## Workflow
1. Detect the packaging stack and current package command.
2. Reuse the real packaging flow where it already works.
3. Fix the packaging flow if the current command is broken.
4. Align preview metadata with the actual output path.
5. Verify that the output directory contains a QA-usable artifact.

## Decision Rules
| Situation | Action |
|---|---|
| Installable package exists | Prefer it |
| Only unpacked runnable app exists | Use it and document how QA should run it |
| Signing/notarization unavailable | Produce unsigned artifact and document the limitation |

## Output Contract
Emit:
- `artifact_preview_status`
- `artifact_build_command`
- `artifact_output_directory`
- `artifact_types`
- `qa_install_notes`

## Related Skills
- `build-artifact`
- `init-desktop-scaffold`
