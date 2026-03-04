---
name: preview-artifact-mobile
description: Use when a mobile project has task preview enabled and the agent must make the post-task preview artifact installable, reproducible, and clear for QA to test on device or simulator.
---

# Mobile Preview Artifact

Use this after `build-artifact` for mobile projects where task preview is delivered as a downloadable artifact.

## Goal
- Produce a QA-usable mobile build from the standard project build pipeline.
- Keep the build command and output directory aligned with ACPMS artifact collection.

## Workflow
1. Identify the actual stack first: React Native, Expo, Flutter, Capacitor, native iOS, native Android, or another mobile toolchain.
2. Reuse the repository's canonical packaging flow when possible. If the current command is broken, fix the repo so packaging is repeatable.
3. Keep metadata aligned with the real mobile output:
   - default command is `npx eas build --local`
   - default output directory is `build`
   - if the project uses a different command or folder, update project metadata
4. Prefer direct tester artifacts:
   - Android: `.apk` first, `.aab` if that is the supported release format
   - iOS: `.ipa` only when signing is available; otherwise document simulator or local signing constraints
5. Make sure the output directory contains at least one artifact or packaged bundle that QA can use.

## Guardrails
- Do not claim iOS installability if signing, provisioning, or notarization is missing.
- Do not switch the task to live preview behavior.
- If only one platform is buildable in the current environment, report the unsupported platform explicitly instead of hiding it.

## Final Report
Include:
- build command used
- output directory
- produced artifact types
- install steps for Android and iOS when relevant
- signing, simulator, or device limitations
