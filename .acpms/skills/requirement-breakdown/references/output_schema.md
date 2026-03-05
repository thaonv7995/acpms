# Output Schema and Quality Checklist

## Drafting schema (internal)

```json
{
  "requirement_intent": "string",
  "current_system_evidence": ["string"],
  "impact_analysis": [
    {
      "area": "backend|frontend|database|api|infra|security|testing|ops|other",
      "impact": "string",
      "risk": "low|medium|high",
      "mitigation": "string"
    }
  ],
  "breakdown_session_task": {
    "title": "[Breakdown] Requirement title",
    "goal": "Analyze requirement and produce execution-ready task plan",
    "task_type": "spike|docs",
    "status": "todo",
    "execution_mode": "analysis_only"
  },
  "implementation_tasks": [
    {
      "title": "string",
      "goal": "string",
      "scope_in": ["string"],
      "scope_out": ["string"],
      "task_type": "feature|bug|refactor|docs|test|chore|hotfix|spike|small_task",
      "depends_on": ["task-title-or-id"],
      "estimate": "S|M|L",
      "definition_of_done": ["string"]
    }
  ],
  "sprint_assignment_recommendation": {
    "mode": "active|selected|null",
    "reason": "string"
  }
}
```

## Quality checklist

- Requirement intent is explicit and concise.
- Evidence points to concrete artifacts (file/endpoint/service/job/config).
- Impact analysis contains only relevant areas.
- Breakdown session task is present and marked analysis-only.
- Implementation tasks are small, reviewable, and non-duplicated.
- Task dependencies are explicit where ordering matters.
- Sprint confirmation question is present.
- All tasks are intended `todo` and not auto-started.

## Post-confirmation payload format

```json
[
  {
    "title": "string",
    "description": "string",
    "task_type": "feature|bug|refactor|docs|test|chore|hotfix|spike|small_task",
    "requirement_id": "uuid",
    "sprint_id": "uuid|null",
    "status": "todo"
  }
]
```

## Constraints

- Implementation tasks count: 3..12.
- If requirement is too broad, split by phase and confirm phase scope first.
- If evidence is missing, state assumptions explicitly before sprint confirmation.
