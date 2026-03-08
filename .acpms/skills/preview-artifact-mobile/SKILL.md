---
name: preview-artifact-mobile
description: Produce QA-usable mobile preview artifacts such as APK, AAB, IPA, or equivalent packaged outputs, with honest installability notes.
---

# Preview Artifact Mobile

## Objective
Generate mobile artifacts that QA can actually use, while being explicit about
platform, signing, and device/simulator limitations.

## When This Applies
- Project type is mobile
- Task preview is delivered as downloadable build output

## Inputs
- Mobile stack
- Build/package command
- Output directory
- Platform signing constraints

## Workflow
1. Detect the mobile stack and real build command.
2. Reuse the project’s packaging flow when it works.
3. Produce Android and/or iOS artifacts appropriate to the environment.
4. Validate that artifacts exist and are usable for QA.
5. Document install constraints honestly.

## Decision Rules
| Situation | Action |
|---|---|
| Android artifact exists | Prefer `.apk` for easiest QA install |
| iOS signing is unavailable | Do not claim device installability |
| Only one platform can be built | Report the unsupported platform clearly |

## Output Contract
Emit:
- `artifact_preview_status`
- `artifact_build_command`
- `artifact_output_directory`
- `artifact_types`
- `qa_install_notes`

## Related Skills
- `build-artifact`
- `init-mobile-scaffold`
