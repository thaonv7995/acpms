---
name: build-artifact
description: Produce deployment-ready artifacts and verify artifact paths for downstream deployment.
---

# Build Artifact

## Objective
Produce the real output that downstream preview or deploy steps depend on, then
prove that the output is usable. This skill is not satisfied by a passing build
command alone. It only succeeds when the expected artifact path exists, matches
the current task flow, and is ready for the next step.

## When This Applies
- ACPMS is preparing for preview, deploy, packaging, release, or artifact download
- The project type expects a generated output such as `dist/`, a binary, an
  archive, or a packaged bundle
- The next skill depends on concrete artifact paths rather than only source code

## Inputs
- Project type: `web`, `api`, `microservice`, `desktop`, `mobile`, `extension`
- Existing build scripts and configuration:
  - `package.json`
  - `Cargo.toml`
  - `pyproject.toml`
  - `Makefile`
  - Docker-related files when build is container-driven
- Expected downstream consumer:
  - local preview runtime
  - artifact preview
  - production deployment
  - review handoff
- Existing output paths if the repo already defines them

## Core Rule
Do not report artifact success from:
- `docker compose config`
- `docker build` without checking produced output
- a build command that exits `0` but leaves no usable artifact
- stale output left over from a previous run without confirming it belongs to the
  current build

## Artifact Expectations By Project Type

| Project Type | Typical Artifact | Notes |
|---|---|---|
| `web` | `dist/`, `.next/`, or framework-specific static/server build output | Must match the preview/deploy path actually used by the repo |
| `api` / `microservice` | built server binary, packaged runtime tree, OpenAPI/Swagger output, or container-ready runtime | Prefer the artifact used by the real runtime path, and expose docs/spec output when the service has an HTTP contract |
| `desktop` | runnable dev app output, unpacked app, packaged bundle, or installer | Prefer the preview surface the environment can actually support, and track Windows/macOS lanes separately when both are in scope |
| `mobile` | dev bundle, simulator-ready output, or packaged app artifact | Prefer the preview surface the environment can actually support, and track Android/iOS lanes separately when both are in scope |
| `extension` | zipped extension, unpacked bundle, or browser-loadable output directory | Prefer the preview surface the browser/toolchain can actually support |

## Build Command Selection
Use the repo's canonical build path if it already exists.

Preferred order:
1. Project-native production build script
2. Framework-native build command already wired in the repo
3. A minimal explicit build command only when no canonical script exists

Examples:
- Node/Web: `npm run build`, `pnpm build`, `yarn build`
- Rust: `cargo build --release`
- Python packaging: project-native package/build command
- Desktop/mobile: project-native package command only if required by task scope

Do not invent an exotic build path when the repo already has one.

## Workflow
1. Inspect the repo and determine the canonical build command.
2. Determine which artifact is actually needed for this task.
   - preview runtime
   - downloadable artifact
   - production deploy
   - review evidence
3. Run the build command in the mode expected by that consumer.
4. Validate the output path:
   - exists
   - is non-empty
   - matches the expected build target
5. If multiple outputs exist, identify which one downstream skills should use.
6. Record the artifact path, type, and any constraints for handoff.

## Validation Checklist
Treat the build as successful only if all relevant checks pass:

- The build command exits successfully
- The expected artifact path exists
- The artifact path is not empty
- The artifact type matches the task flow
- The artifact is fresh enough to be trusted for the current run

Optional but recommended:
- For web builds, inspect the output root for `index.html` or equivalent
- For API builds, confirm the artifact supports the actual runtime path:
  - entry binary or server bundle exists
  - container runtime can reference the built output
  - generated API docs/spec output exists if the repo expects it
- For desktop builds, confirm the output matches the intended preview surface:
  - runnable dev app path exists when dev-run preview is expected
  - unpacked app directory exists when packaging stops short of an installer
  - installer or bundle exists when full packaging succeeds
  - Windows and macOS outputs are reported separately when both lanes are in scope
- For mobile builds, confirm the output matches the intended preview surface:
  - Expo/dev bundle metadata exists when using dev bundle preview
  - Android artifact exists for installable QA preview
  - iOS artifact is only reported when signing/export really succeeded
  - Android and iOS outputs are reported separately when both lanes are in scope
- For extension builds, confirm the output matches the intended preview surface:
  - zip exists when downloadable package preview is expected
  - unpacked directory contains manifest and required runtime assets
- For packaged artifacts, confirm the main bundle/file exists
- For deploy-targeted output, verify the path is the one the next deploy skill expects

## Decision Rules
| Situation | Action |
|---|---|
| Multiple build targets exist | Build only the target relevant to the current flow and report which one was selected. |
| API preview/deploy runs from a containerized service | Build the output that the container really uses, not only a library/test artifact. |
| API or microservice exposes Swagger/OpenAPI or equivalent docs route | Treat docs/spec output as part of the previewable artifact set, not optional fluff. |
| API has helper services but app build is stateless | Build the app artifact and let compose/runtime skills handle support services separately. |
| Desktop app preview is a runnable dev app or unpacked app | Prefer the build path that matches that preview mode instead of forcing a signed installer. |
| Desktop release packaging requires unavailable signing/notarization | Report the limitation honestly rather than claiming a releasable installer exists. |
| Mobile app preview is simulator/dev-bundle based | Prefer the bundle/build path that matches that preview mode instead of forcing a full release package. |
| Mobile release artifact requires unavailable signing or platform tooling | Report the limitation honestly rather than claiming a releasable artifact exists. |
| Extension preview is artifact-based | Prefer zip when supported; otherwise validate and report the unpacked extension directory. |
| Build succeeds but output path is missing | Treat it as a failed artifact build. |
| Build tooling is missing | Stop and report the missing setup requirement. |
| Output exists but is obviously stale or wrong | Rebuild or fail explicitly; do not hand off a bad artifact. |
| Task does not require an artifact | Skip this skill rather than forcing a meaningless build. |
| Docs-only or tiny metadata-only task | Prefer not to run this skill unless the workflow explicitly requires it. |

## Output Contract
Emit a `Build Artifact Summary` with:
- `build_status`: `success` | `failed` | `skipped`
- `build_command`: exact command used
- `artifact_paths`: list of produced paths actually relevant to the task
- `platform_artifacts`: per-platform lanes when mobile or desktop targets multiple OSes
- `artifact_notes`: short note on artifact type and downstream usage

If build is skipped, include a short reason.

## Good Output Example

```md
Build Artifact Summary
- build_status: success
- build_command: npm run build
- artifact_paths:
  - dist/
- artifact_notes: Static web build for preview and deploy
```

```md
Build Artifact Summary
- build_status: success
- build_command: cargo build --release
- artifact_paths:
  - target/release/api-server
- artifact_notes: API runtime binary used by Docker preview and deploy
```

```md
Build Artifact Summary
- build_status: success
- build_command: npm run package
- artifact_paths:
  - dist/MyDesktopApp.app
- artifact_notes: Desktop packaged bundle used for QA preview
```

```md
Build Artifact Summary
- build_status: success
- build_command: npx expo export
- artifact_paths:
  - dist/
- artifact_notes: Expo dev bundle/export used for mobile preview
```

```md
Build Artifact Summary
- build_status: success
- build_command: npm run build:extension
- artifact_paths:
  - dist-extension/
- artifact_notes: Unpacked browser extension directory used for QA loading
```

## Bad Output Example

```md
Build passed.
```

This is insufficient because it does not prove that any downstream artifact exists.

## Related Skills
- `verify-test-build`
- `preview-docker-runtime`
- `deploy-cloudflare-pages`
- `preview-artifact-desktop`
- `preview-artifact-mobile`
- `preview-artifact-extension`
