---
name: requirement-breakdown
description: Analyze a requirement, gather concrete evidence, propose a bounded implementation plan, and prepare ACPMS-compatible task payloads after sprint confirmation.
---

# Requirement Breakdown

## Objective
Turn one requirement into a focused implementation plan with evidence, impact
analysis, a dedicated breakdown task, and clean proposed tasks that ACPMS can
turn into work items.

## When This Applies
- User asks to break down a requirement into tasks
- User wants impact analysis before task creation
- User wants structured task proposals tied to sprint planning

## Inputs
- Requirement text
- Relevant codebase/system evidence
- Current sprint context, if known

## Workflow
1. Clarify the requirement intent and success criteria.
2. Inspect only the system evidence relevant to that requirement.
3. Write a concise impact analysis.
4. Propose one analysis-only breakdown task.
5. Propose implementation tasks with scope, estimate, and definition of done.
6. Ask for sprint assignment confirmation before final task-creation payloads.

## Decision Rules
| Situation | Action |
|---|---|
| Critical context is missing | Ask a few targeted questions, then continue with explicit assumptions |
| Requirement is docs-only | Prefer `docs` or `spike` where appropriate |
| Requirement is broad | Break into 3-12 bounded implementation tasks |
| Sprint is not confirmed | Stop before emitting final create-task payloads |

## Output Contract
Use this section order:
1. `Requirement intent`
2. `Current-system evidence`
3. `Impact analysis`
4. `Breakdown session task (analysis-only)`
5. `Implementation tasks (proposed)`
6. `Sprint assignment (confirmation required)`

Realtime stream contract:
- emit `BREAKDOWN_TASK { ... }` lines as drafts become ready
- end with the final structured output or task payload proposal

## Guardrails
- No filler or repeated restatement
- Every proposed task must map to a concrete impact item
- Do not auto-start tasks from this flow

## Related Skills
- `project-assistant`
- `init-import-analyze`
- `task-preflight-check`
