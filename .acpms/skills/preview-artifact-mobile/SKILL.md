---
name: preview-artifact-mobile
description: Produce QA-usable mobile preview artifacts such as APK, AAB, IPA, or equivalent packaged outputs, with honest installability notes.
---

# Preview Artifact Mobile

## Objective
Generate mobile artifacts that QA can actually use, while being explicit about
platform, signing, and device/simulator limitations.

Mobile preview is not a browser URL by default. Prefer the most honest preview
surface for the current stack and environment: simulator/emulator run, Expo/dev
bundle, or installable artifact.

## When This Applies
- Project type is mobile
- Task preview is delivered as downloadable build output

## Inputs
- Mobile stack
- Build/package command
- Output directory
- Platform signing constraints
- Available preview surfaces:
  - simulator/emulator
  - Expo/dev client
  - Android artifact
  - iOS artifact
  - screenshots or evidence captured from a successful run

## Workflow
1. Detect the mobile stack and the real preview path for that stack.
2. Prefer the best preview surface available in this order:
   - simulator/emulator run
   - Expo/dev client or equivalent dev bundle
   - Android artifact (`.apk`) or other installable package
   - iOS artifact only when signing/export is actually available
3. Reuse the project’s native packaging flow when it works.
4. Produce the artifact or preview bundle appropriate to the environment.
5. Validate that the result exists and is usable for QA.
6. Document installability, simulator limitations, and signing constraints
   honestly.

## Decision Rules
| Situation | Action |
|---|---|
| Expo or equivalent dev bundle is the main preview path | Report that preview path instead of pretending there is a web URL |
| Android artifact exists | Prefer `.apk` for easiest QA install |
| iOS signing is unavailable | Do not claim device installability |
| Only one platform can be built | Report the unsupported platform clearly |
| No installable artifact can be produced but simulator/dev bundle works | Treat simulator/dev bundle as the preview surface and report it clearly |

## Output Contract
Emit:
- `artifact_preview_status`
- `artifact_build_command`
- `artifact_output_directory`
- `artifact_types`
- `preview_surface`: `simulator` | `emulator` | `dev_bundle` | `android_artifact` | `ios_artifact`
- `qa_install_notes`

## Related Skills
- `build-artifact`
- `init-mobile-scaffold`
