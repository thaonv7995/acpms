---
name: init-import-analyze
description: Analyze an imported repository and write `.acpms/import-analysis.json` so ACPMS understands the architecture, services, and current technical state before planning further work.
---

# Init Import Analyze

## Objective
Turn a cloned/imported repository into a structured ACPMS architecture summary
without modifying application code or guessing beyond available evidence.

## When This Applies
- ACPMS cloned an existing repository
- The init flow is import-based rather than from-scratch
- Architecture mapping is needed before planning tasks or generating PRD context

## Inputs
- Current repository contents
- Key manifests, configs, and entry points
- Detected project type or service topology

## Workflow
1. Inspect root files and key subdirectories.
2. Identify deployable units and major components.
3. Infer the architecture graph from manifests, configs, and source structure.
4. Summarize tech stack, services, and current project shape.
5. Write `.acpms/import-analysis.json` with valid JSON.

## Decision Rules
| Situation | Action |
|---|---|
| Frontend-only app | Keep the graph minimal |
| Full-stack app | Include frontend, API, database, auth, and storage when evidenced |
| Monorepo | Model each deployable service separately |
| Structure is unclear | Infer conservatively and explain uncertainty in summary |

## Output Contract
Create `.acpms/import-analysis.json` with:
- `architecture.nodes`
- `architecture.edges`
- `assessment.project_type`
- `assessment.summary`
- `assessment.services`
- `assessment.tech_stack`

## Guardrails
- Read-only analysis; do not modify source files
- Prefer static analysis over install/build
- If analysis is partial, still write valid JSON with the best supported summary

## Related Skills
- `init-read-references`
- `init-project-context-file`
- `requirement-breakdown`
