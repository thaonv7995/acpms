# OpenClaw Gateway: 06 - API Design Standards (Status & Error Codes)

## 1. RESTful Response Standardization

To ensure maximum compatibility with OpenClaw while preserving parity with the real product contract, the gateway should **reuse the same response and error shapes as the internal API wherever possible**.

### 1.1 Success Response Wrapper
Mirrored OpenClaw endpoints should not invent a new "simplified" success format. The preferred standard is to preserve the existing internal API response envelope.

In the current backend, successful REST responses use the `ApiResponse<T>` wrapper:

```json
{
  "success": true,
  "code": "0000",
  "message": "Projects retrieved successfully",
  "data": [
    {
      "id": "proj_123",
      "name": "Backend Refactor"
    }
  ]
}
```

This same wrapper should be preserved for mirrored OpenClaw REST endpoints unless a specific internal endpoint already returns a different shape.

### 1.2 Error Response Wrapper
When an API call fails, the gateway should preserve the existing internal `ApiError -> ApiResponse<()>` mapping instead of creating a second OpenClaw-only error schema.

**Preferred Error Shape:**
```json
{
  "success": false,
  "code": "4010",
  "message": "Authentication failed",
  "error": {
    "details": "Authentication failed",
    "trace_id": null
  }
}
```

Gateway-specific auth failures should still use the same global envelope, with details that explain the OpenClaw-specific failure.

## 2. Global HTTP Status Codes

The gateway relies on standard HTTP semantics. Agents interacting with the mirrored API must interpret the HTTP status code first, exactly as they would with the internal API.

| Status Code | Meaning | Use Case |
| :--- | :--- | :--- |
| **200 OK** | Success | Standard response for successful `GET`, `PUT`, `POST` (if not creating), `DELETE` (if returning data). |
| **201 Created** | Resource Created | Returned when a mirrored `POST` creates a new entity. |
| **400 Bad Request** | Validation Error | The JSON payload was malformed, missing required fields, or violated business logic rules. |
| **401 Unauthorized** | Auth Failure | Missing, invalid, or expired `Authorization: Bearer <TOKEN>`. |
| **403 Forbidden** | Gateway Disabled / Explicitly Blocked | The token is valid, but the OpenClaw gateway is disabled or a route is intentionally blocked from external automation. |
| **404 Not Found** | Resource Missing| The requested Project ID, Task ID, or Attempt ID does not exist. |
| **409 Conflict** | State Conflict | The operation is valid syntactically but conflicts with current resource state. |
| **429 Too Many Requests** | Rate Limited | OpenClaw has exceeded its allowed quota of API calls per minute. |
| **500 Internal Server Error** | System Failure | Agentic-Coding experienced an unexpected panic or database failure. |
| **503 Service Unavailable** | Overloaded | The orchestrator queue is completely full; no new sessions can be triggered. |

## 3. Specific Error Codes (Payload `code`)

To help OpenClaw recover from failures programmatically, the gateway should continue using the internal response-code families wherever possible.

### 3.1 Authentication & Authorization
*   `4010` / `Unauthorized`: No valid `Authorization` header was provided. *(Action: Attach or rotate the token).*
*   `4030` / `Forbidden`: The gateway is disabled or the endpoint is intentionally blocked from OpenClaw. *(Action: Stop retrying and alert the operator).*

### 3.2 Resource Errors
*   `4040` / `NotFound`: General 404.
*   `4042` / `ProjectNotFound`: The targeted `project_id` is invalid.
*   `4041` / `TaskNotFound`: The targeted `task_id` does not exist.
*   Other internal resource codes should be preserved when available.

### 3.3 Orchestration Errors
*   `5003` / `ServiceUnavailable`: The system is temporarily overloaded. *(Action: Retry with backoff).*
*   `4090` / `Conflict`: Attempted an operation that is incompatible with the current state.
*   Existing endpoint-specific messages should be preserved in `message` / `error.details`.

### 3.4 Validation Errors
*   `4001` / `ValidationError`: Payload schema mismatch or invalid field constraints.
    ```json
    {
      "success": false,
      "code": "4001",
      "message": "Invalid request payload.",
      "error": {
        "details": "Validation error: title is required",
        "trace_id": null
      }
    }
    ```

## 4. Super Admin Authorization Semantics

Because OpenClaw is intentionally treated as a Super Admin integration:

*   project-level permission failures should be rare after successful gateway authentication
*   a `403` generally indicates a gateway-level policy decision, not ordinary project RBAC
*   every request should still be audited as having originated from the OpenClaw principal

## 5. OpenAPI / Swagger Documentation Enforcement

All mirrored status codes and error responses **must** be explicitly documented in `utoipa` so that OpenClaw's generated tools match the real backend behavior.

**Rust `utoipa` Implementation Example:**
```rust
#[utoipa::path(
    post,
    path = "/api/openclaw/v1/tasks/{task_id}/attempts",
    request_body = CreateAttemptPayload,
    responses(
        (status = 201, description = "Attempt created", body = ApiResponseTaskAttemptDto),
        (status = 400, description = "Validation failed", body = ApiResponseEmpty),
        (status = 401, description = "Invalid API Key", body = ApiResponseEmpty),
        (status = 403, description = "Gateway disabled or blocked", body = ApiResponseEmpty),
        (status = 409, description = "State conflict", body = ApiResponseEmpty)
    ),
    security(
        ("openclaw_api_key" = [])
    )
)]
pub async fn create_attempt(...) { ... }
```
This ensures the `/api/openclaw/openapi.json` contract stays aligned with the real internal API surface that OpenClaw is meant to operate.
