---
name: requirement-breakdown
description: Analyze a requirement and break it into focused tasks with evidence-based impact analysis, a dedicated AI breakdown session task, and a mandatory sprint confirmation gate before task creation.
---

# Requirement Breakdown

## Objective
Convert one requirement into a clear, implementable task plan for Vibe-Kanban without rambling.

## When this applies
- User asks to break/split requirement into tasks.
- User asks for impact analysis before task creation.
- User wants tasks attached to current sprint or user-selected sprint.

## Mandatory workflow
1. Clarify requirement intent and success criteria.
2. Analyze current system evidence (files/endpoints/services/configs).
3. Produce impact analysis (only relevant areas).
4. Create one dedicated **AI breakdown session task** (analysis-only, not execution).
5. Propose implementation tasks (small, reviewable, todo).
6. Ask user to confirm sprint assignment.
7. Only after confirmation, produce final task-creation payload(s).

If critical context is missing, ask at most 3 targeted questions, then continue with explicit assumptions.

## AI breakdown session task (required)
- This task belongs to Vibe-Kanban but is **not** an execution task.
- Default `task_type`: `spike`.
- Use `docs` only when the requirement is documentation-only.
- If a dedicated `analysis` type is introduced later, prefer `analysis`.
- Status must remain `todo`; do not auto-start or trigger coding attempt from this task.
- Purpose: capture requirement analysis, impact summary, and proposed execution plan.

Suggested title pattern:
- `[Breakdown] <requirement title>`

## Output contract
Follow this section order exactly:

1. `Requirement intent`
2. `Current-system evidence`
3. `Impact analysis`
4. `Breakdown session task (analysis-only)`
5. `Implementation tasks (proposed)`
6. `Sprint assignment (confirmation required)`

For each proposed implementation task include:
- `title`
- `goal` (1 sentence)
- `scope` (in/out)
- `task_type` (`feature|bug|refactor|docs|test|chore|hotfix|spike|small_task`)
- `depends_on` (optional)
- `estimate` (`S|M|L`)
- `definition_of_done` (1-3 bullets)

## Mandatory confirmation gate
Before outputting create-task payloads, ask user to confirm one option:
- Assign all tasks to active/current sprint (recommended).
- Assign all tasks to a specific sprint selected by user.
- Leave sprint empty (`null`) for backlog.

Do not skip this gate.

## Task payload constraints
- Breakdown session task + implementation tasks are all created with `status: "todo"`.
- No task in this flow is auto-started.
- Keep proposed tasks between 3 and 12 (implementation tasks only).

Reference schema/checklist: [`references/output_schema.md`](references/output_schema.md)

## Anti-rambling guardrails
- No generic filler or repeated restatement.
- Keep explanations concise and evidence-based.
- Every task must map to a concrete impact item.
- Use the same language as the user unless they request otherwise.
