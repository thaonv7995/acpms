---
name: preview-artifact-desktop
description: Produce a QA-usable desktop build artifact for task preview, aligned with the project’s real packaging flow and output paths.
---

# Preview Artifact Desktop

## Objective
Generate a downloadable desktop artifact that QA can actually run or install,
without confusing artifact preview with live URL preview.

Desktop preview is not a browser URL by default. Prefer the most honest preview
surface for the current stack and environment: runnable dev app, unpacked app,
or packaged installer/bundle.

Treat desktop delivery as platform-specific lanes whenever the product targets
more than one OS:
- Windows
- macOS

## When This Applies
- Project type is desktop
- Task preview should be delivered as downloadable artifacts

## Inputs
- Desktop packaging stack
- Actual package/build command
- Real output directory
- Available preview surfaces:
  - runnable dev app
  - unpacked runnable app
  - packaged installer or bundle
  - screenshots or evidence captured from a successful run

## Workflow
1. Detect the packaging stack and the real preview surface for that stack.
2. Prefer the best preview surface available in this order:
   - runnable dev app when that is the intended validation path
   - unpacked runnable app
   - packaged installer or bundle
3. Reuse the real packaging flow where it already works.
4. Fix the packaging flow if the current command is broken.
5. Align preview metadata with the actual output path.
6. Validate each platform lane separately:
   - Windows lane
   - macOS lane
7. Verify that the output directory contains a QA-usable artifact or that the
   runnable app path really works.

## Decision Rules
| Situation | Action |
|---|---|
| Dev run is the only realistic preview path in this environment | Report the runnable app path instead of pretending an installer exists |
| Installable package exists | Prefer it |
| Only unpacked runnable app exists | Use it and document how QA should run it |
| Signing/notarization unavailable | Produce unsigned artifact and document the limitation |
| Windows and macOS are both in scope | Always report both lanes, even if one ends as `unavailable_in_current_environment` |
| Platform packaging is unsupported in the current environment | Report the limitation and fall back to the nearest truthful preview surface |

## Output Contract
Emit:
- `artifact_preview_status`
- `artifact_build_command`
- `artifact_output_directory`
- `artifact_types`
- `preview_surface`: `dev_app` | `unpacked_app` | `installer` | `bundle`
- `windows_artifact_status`
- `windows_artifact_path`
- `macos_artifact_status`
- `macos_artifact_path`
- `qa_install_notes`

## Related Skills
- `build-artifact`
- `init-desktop-scaffold`
