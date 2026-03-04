---
name: preview-artifact-extension
description: Use when a browser extension project has task preview enabled and the agent must make the post-task preview artifact downloadable, loadable in the browser, and predictable for QA.
---

# Extension Preview Artifact

Use this after `build-artifact` for extension projects where task preview is delivered as a downloadable bundle rather than a live URL.

## Goal
- Produce a browser-loadable extension preview package from the normal build pipeline.
- Keep ACPMS artifact collection aligned with the real extension output.

## Workflow
1. Detect the build stack first: raw WebExtension, Plasmo, WXT, Vite-based extension tooling, or another framework.
2. Reuse the current extension build if it works. If it does not, fix the repository so the standard build command succeeds reliably.
3. Keep metadata aligned with the real output:
   - default command is `npm run build:ext`
   - default output directory is `ext`
   - if the project emits to another path, update project metadata
4. Prefer a ready-made `.zip` when the toolchain already produces one. Otherwise make sure the built extension directory is complete so ACPMS can zip it for download.
5. Verify the built output contains the files QA needs:
   - manifest
   - background/service worker bundle when applicable
   - popup/options/content script assets when applicable

## Guardrails
- Do not treat extension preview as a live preview URL.
- Do not ship only source files when a built bundle is expected.
- Preserve browser-specific manifest requirements and document any unsupported browser targets.

## Final Report
Include:
- build command used
- output directory
- whether QA should use the generated zip or unpacked directory
- browser load steps
- browser-specific limitations
