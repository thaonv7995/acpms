---
name: preview-artifact-desktop
description: Use when a desktop project has task preview enabled and the agent must make the post-task preview artifact installable, reproducible, and easy for QA to download and test.
---

# Desktop Preview Artifact

Use this after `build-artifact` for desktop projects where task preview is delivered as a downloadable artifact, not a live URL.

## Goal
- Make sure the repository can produce a stable desktop preview package from the standard build pipeline.
- Keep the package discoverable by ACPMS through the configured build command and output directory.

## Workflow
1. Inspect the existing packaging stack first: Electron, Tauri, Wails, Flutter Desktop, or native tooling.
2. Reuse the current packaging command when it already works. If it does not, fix the repo so the canonical build command succeeds.
3. Make sure the project metadata stays aligned with the real package output:
   - default command is `npm run package`
   - default output directory is `out`
   - if the project uses another command or folder, update project metadata instead of relying on guesswork
4. Prefer installable outputs when the stack supports them:
   - macOS: `.dmg` or `.pkg`
   - Windows: `.exe` or `.msi`
   - otherwise keep a runnable packaged app inside the output directory
5. Validate that the output directory is non-empty and contains at least one QA-usable build.

## Guardrails
- Do not convert desktop preview into a live preview URL.
- Do not leave packaging dependent on manual local steps that the build pipeline cannot repeat.
- If signing or notarization is unavailable, say so clearly and still produce the best unsigned QA artifact possible.

## Final Report
Include:
- build command used
- output directory
- artifact types produced
- platform coverage
- install notes or known limitations for testers
