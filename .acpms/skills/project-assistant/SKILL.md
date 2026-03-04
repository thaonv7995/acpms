---
name: project-assistant
description: Project Assistant tool contract for create_requirement and create_task. Output must follow this schema for backend confirm-tool.
---

# Project Assistant Tool Contract

## Objective
When the Project Assistant proposes creating a Requirement or Task, it MUST output a JSON object in the exact format below. The backend parses this to show a confirmation card to the user.

## Output Format (one JSON object per line)

### create_requirement

```json
{"tool":"create_requirement","args":{"title":"string","content":"string","priority":"low|medium|high|critical"}}
```

| Field | Required | Default |
|-------|----------|---------|
| title | ✓ | - |
| content | ✓ | - |
| priority | | medium |

### create_task

```json
{"tool":"create_task","args":{"title":"string","description":"string","task_type":"feature|bug|refactor|docs|test|chore|hotfix|spike|small_task","requirement_id":"uuid|null","sprint_id":"uuid|null"}}
```

| Field | Required | Default |
|-------|----------|---------|
| title | ✓ | - |
| description | | null |
| task_type | ✓ | feature |
| requirement_id | | null |
| sprint_id | | null |

**task_type** must be one of: feature, bug, refactor, docs, test, chore, hotfix, spike, small_task.

## Rules
- Output exactly one JSON object per line when proposing a tool call.
- You may precede with natural language (e.g., "I suggest creating this requirement:").
- The JSON line will be parsed by the backend; invalid JSON is ignored.
- User must confirm before the tool is executed.
