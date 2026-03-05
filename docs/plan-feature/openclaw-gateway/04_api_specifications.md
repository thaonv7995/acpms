# OpenClaw Gateway: 04 - API Specifications

This document outlines the initial set of endpoints to be implemented within the `/api/openclaw/v1` namespace. They are designed concisely, preferring flat JSON payloads optimized for LLM readability.

All endpoints (except `openapi.json`) require:
`Authorization: Bearer <OPENCLAW_API_KEY>`

---

## 1. Data Exposure (Read APIs)

### 1.1 List Projects
*   **Endpoint**: `GET /api/openclaw/v1/projects`
*   **Purpose**: Allows OpenClaw to discover valid project IDs and basic repository metadata.
*   **Returns**: An array of lightweight project objects.

### 1.2 Get Kanban State
*   **Endpoint**: `GET /api/openclaw/v1/projects/{project_id}/kanban`
*   **Purpose**: Retrieves the current workflow state of a specific project. Crucial for the AI to understand what tasks exist, what is `In-progress`, and what needs attention.
*   **Returns**: Task metadata grouped by status columns (Todo, In-progress, In-review, Done).

### 1.3 Get Orchestrator/Session Status
*   **Endpoint**: `GET /api/openclaw/v1/sessions/{session_id}`
*   **Purpose**: Checks the precise status of a running executor instance. Note: While Webhooks push this data aggressively, a fallback GET request is standard for recovery.
*   **Returns**: Session status enum (Starting, Running, Paused, Completed, Failed), current logs/context buffer.

---

## 2. Control Plane (Write/Execute APIs)

### 2.1 Create Task
*   **Endpoint**: `POST /api/openclaw/v1/projects/{project_id}/tasks`
*   **Purpose**: OpenClaw parses a user requirement and breaks it down, creating actionable tasks inside Agentic-Coding.
*   **Payload**:
    ```json
    {
      "title": "Implement Webhook Dispatcher",
      "description": "Create the HMAC signing logic in Rust...",
      "priority": "High"
    }
    ```
*   **Returns**: The newly created `Task` object, including its assigned `task_id`.

### 2.2 Trigger Executor Session
*   **Endpoint**: `POST /api/openclaw/v1/orchestrator/trigger`
*   **Purpose**: Overrides or injects a command, ordering the internal `Agentic-Coding` orchestrator to wake up an executor (e.g., Claude, local bash env) and resolve a specific task.
*   **Payload**:
    ```json
    {
      "project_id": "uuid-v4...",
      "task_id": "uuid-v4...",
      "instructions": "Use the provided task description. Focus heavily on security tests.",
      "agent_type": "claude-code" // Optional: specify executor backend
    }
    ```
*   **Returns**: A `session_id` to track progress. A Webhook will be fired later when this session updates.

### 2.3 Stop/Pause Session (Emergency Stop)
*   **Endpoint**: `POST /api/openclaw/v1/sessions/{session_id}/pause`
*   **Purpose**: Gives OpenClaw control over runaway processes or erroneous scripts executed by the internal agents.
