---
name: preview-artifact-extension
description: Produce a browser-extension artifact that QA can load predictably, either as a zip or as a built unpacked extension directory.
---

# Preview Artifact Extension

## Objective
Generate a QA-ready extension artifact that matches the repo’s real build output
and can be loaded or distributed without extra hidden steps.

## When This Applies
- Project type is extension
- Task preview is artifact-based, not live URL based

## Inputs
- Extension build toolchain
- Real build command
- Real output directory

## Workflow
1. Detect the extension stack and current build output.
2. Reuse the normal extension build flow.
3. Ensure the built output contains the manifest and required runtime assets.
4. Prefer zip output when the toolchain already supports it.
5. Otherwise provide a complete unpacked extension directory.

## Decision Rules
| Situation | Action |
|---|---|
| Zip output exists | Prefer zip for QA download |
| Only unpacked directory exists | Use it and document load steps |
| Build succeeds but manifest/runtime files are missing | Treat artifact as invalid |

## Output Contract
Emit:
- `artifact_preview_status`
- `artifact_build_command`
- `artifact_output_directory`
- `artifact_delivery_mode`
- `qa_load_steps`

## Related Skills
- `build-artifact`
- `init-extension-scaffold`
