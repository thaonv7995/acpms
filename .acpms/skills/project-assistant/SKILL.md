---
name: project-assistant
description: Output valid confirmation-tool JSON for creating requirements and tasks in ACPMS, with clear intent, safe defaults, and no malformed payloads.
---

# Project Assistant

## Objective
Generate machine-parseable tool proposals for ACPMS project management actions
without producing malformed JSON or ambiguous task metadata.

## When This Applies
- The agent wants to propose creating a requirement
- The agent wants to propose creating a task
- ACPMS should show a confirmation card before executing the action

## Inputs
- User request
- Requirement/task details already gathered
- Optional sprint or requirement linkage

## Workflow
1. Decide whether the proposal is for `create_requirement` or `create_task`.
2. Gather the minimum required fields.
3. Apply safe defaults when optional fields are missing.
4. Emit one valid JSON object per line for the backend parser.
5. Do not emit invalid JSON or unsupported task types.

## Decision Rules
| Situation | Action |
|---|---|
| Requirement proposal | Use `create_requirement` schema |
| Task proposal | Use `create_task` schema |
| Optional field unavailable | Use `null` or the documented default |
| Multiple proposals needed | Emit one JSON object per line |

## Output Contract
Valid requirement payload:

```json
{"tool":"create_requirement","args":{"title":"string","content":"string","priority":"low|medium|high|critical"}}
```

Valid task payload:

```json
{"tool":"create_task","args":{"title":"string","description":"string","task_type":"feature|bug|refactor|docs|test|chore|hotfix|spike|small_task","requirement_id":"uuid|null","sprint_id":"uuid|null"}}
```

## Guardrails
- Output valid JSON only
- Use supported `task_type` values only
- Do not assume execution happens immediately; user confirmation is required

## Related Skills
- `requirement-breakdown`
