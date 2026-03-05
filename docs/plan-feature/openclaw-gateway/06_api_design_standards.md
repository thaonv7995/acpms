# OpenClaw Gateway: 06 - API Design Standards (Status & Error Codes)

## 1. RESTful Response Standardization

To ensure maximum compatibility with OpenClaw and other automated LLM-based agents, the gateway will enforce a strict, predictable JSON response format for all API endpoints.

### 1.1 Success Response Wrapper
Successful queries should directly return the requested resource (flat object or array) without unnecessary meta-wrappers, adhering to OpenAPI best practices for LLM function calling.

**Example (GET /projects):**
```json
[
  {
    "id": "proj_123",
    "name": "Backend Refactor",
    "status": "active"
  }
]
```

### 1.2 Error Response Wrapper
When an API call fails, the gateway will **always** return a standardized JSON error object containing a deterministic error code, a human-readable message, and optional detailed context.

**Schema:**
```json
{
  "error": {
    "code": "ERROR_CODE_STRING",
    "message": "Human readable explanation of the failure.",
    "details": {} // Optional object with specific validation errors
  }
}
```

## 2. Global HTTP Status Codes

The gateway relies on standard HTTP semantics. Agents interacting with the API must interpret the HTTP Status Code first.

| Status Code | Meaning | Use Case |
| :--- | :--- | :--- |
| **200 OK** | Success | Standard response for successful `GET`, `PUT`, `POST` (if not creating), `DELETE` (if returning data). |
| **201 Created** | Resource Created | Returned when `POST /tasks` or `POST /orchestrator/trigger` successfully generates a new entity. |
| **400 Bad Request** | Validation Error | The JSON payload was malformed, missing required fields, or violated business logic rules. |
| **401 Unauthorized** | Auth Failure | Missing, invalid, or expired `Authorization: Bearer <TOKEN>`. |
| **403 Forbidden** | Access Denied | The Token is valid, but OpenClaw lacks the specific role/permission to perform the action. |
| **404 Not Found** | Resource Missing| The requested Project ID, Task ID, or Session ID does not exist. |
| **429 Too Many Requests** | Rate Limited | OpenClaw has exceeded its allowed quota of API calls per minute. |
| **500 Internal Server Error** | System Failure | Agentic-Coding experienced an unexpected panic or database failure. |
| **503 Service Unavailable** | Overloaded | The orchestrator queue is completely full; no new sessions can be triggered. |

## 3. Specific Error Codes (Payload `code`)

To help the OpenClaw Agent autonomously recover from errors, the `"code"` field in the error payload will map to specific, actionable issues.

### 3.1 Authentication & Authorization
*   `AUTH_MISSING_TOKEN`: No `Authorization` header found. *(Action: OpenClaw must attach the token).*
*   `AUTH_INVALID_TOKEN`: The provided token does not match `OPENCLAW_API_KEY`. *(Action: Stop retrying, alert admin).*

### 3.2 Resource Errors
*   `RESOURCE_NOT_FOUND`: General 404.
*   `PROJECT_NOT_FOUND`: The targeted `project_id` is invalid.
*   `TASK_NOT_FOUND`: The targeted `task_id` does not exist.

### 3.3 Orchestration Errors
*   `ORCHESTRATOR_BUSY`: The system cannot start a new session because the maximum concurrency limit is reached. *(Action: OpenClaw should retry with exponential backoff).*
*   `SESSION_ALREADY_RUNNING`: Attempted to trigger a task that is already actively being executed by another agent session.
*   `INVALID_TRANSITION`: Attempted to pause a session that is already 'completed', or resume a session that is 'failed'.

### 3.4 Validation Errors
*   `VALIDATION_FAILED`: Payload schema mismatch. The `details` object will contain the exact field names.
    ```json
    {
      "error": {
        "code": "VALIDATION_FAILED",
        "message": "Invalid request payload.",
        "details": {
          "title": "Field is required and cannot be empty."
        }
      }
    }
    ```

## 4. OpenAPI / Swagger Documentation Enforcement

All status codes and error responses **must** be explicitly documented in the `utoipa` macros so that OpenClaw's code generator expects them.

**Rust `utoipa` Implementation Example:**
```rust
#[utoipa::path(
    post,
    path = "/api/openclaw/v1/orchestrator/trigger",
    request_body = TriggerPayload,
    responses(
        (status = 201, description = "Session successfully triggered", body = SessionResponse),
        (status = 400, description = "Validation failed", body = ErrorResponse, example = json!({"error": {"code": "VALIDATION_FAILED", "message": "Missing task_id"}})),
        (status = 401, description = "Invalid API Key", body = ErrorResponse),
        (status = 503, description = "Orchestrator queue is full", body = ErrorResponse, example = json!({"error": {"code": "ORCHESTRATOR_BUSY"}}))
    ),
    security(
        ("openclaw_api_key" = [])
    )
)]
pub async fn trigger_session(...) { ... }
```
This ensures the `/api/openclaw/openapi.json` contract is strictly typed, allowing OpenClaw to write deterministic generic-error-handling routines when calling Agentic-Coding.
