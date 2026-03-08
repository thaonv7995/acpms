---
name: init-mobile-scaffold
description: Create a mobile application baseline with platform-aware structure, run instructions, and the minimum setup needed for simulator/device development.
---

# Init Mobile Scaffold

## Objective
Bootstrap a mobile app that can move quickly into real feature work without
hiding platform constraints around Android, iOS, signing, or simulator setup.

## When This Applies
- Project type is mobile
- ACPMS is creating a new mobile app baseline

## Inputs
- Project brief
- Requested stack or framework
- Target platform assumptions

## Workflow
1. Choose the mobile stack that matches explicit requirements.
2. Create the core app entrypoint and app structure.
3. Add platform-aware config and environment stubs.
4. Add README and developer run instructions.
5. Leave the project in a state that can be built or run in simulator/emulator.

## Required Baseline
- app source entrypoint
- platform config
- README
- `.gitignore`
- build/run command path

## Decision Rules
| Situation | Action |
|---|---|
| Cross-platform framework is unspecified | Choose the safest default for fast iteration |
| iOS signing is unavailable | Document the limitation, do not fake installability |
| One platform is out of scope | Be explicit rather than pretending dual-platform parity |

## Output Contract
Emit:
- `scaffold_status`
- `selected_stack`
- `created_files`
- `platform_support`
- `verification_commands`

## Related Skills
- `init-project-bootstrap`
- `preview-artifact-mobile`
- `verify-test-build`
