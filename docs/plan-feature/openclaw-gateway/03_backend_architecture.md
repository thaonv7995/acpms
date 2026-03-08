# OpenClaw Gateway: 03 - Backend Architecture (Rust)

## 1. Structural Organization (`crates/server`)

To ensure clean code separation, the API routes dedicated to external integrations (OpenClaw) will be separated from the internal frontend APIs while still reusing the same handlers and domain services.

*   **Module Path**: Create a new module directory at `crates/server/src/routes/openclaw/`.
*   **REST/SSE Base URI**: All mirrored gateway APIs will be prefixed with `/api/openclaw/v1`.
*   **WebSocket Base URI**: Mirrored WebSocket routes should be exposed under `/api/openclaw/ws`.
*   **Core Rule**: The gateway should mirror the internal API surface instead of re-implementing business logic in bespoke OpenClaw-only handlers.

### Directory Structure Mockup:
```text
crates/server/src/routes/
├── ...
├── openclaw/
│   ├── mod.rs          // Gateway route registration and mirroring
│   ├── auth.rs         // Bearer token validation + synthetic Super Admin identity
│   ├── audit.rs        // Audit metadata, request tracing, actor annotations
│   ├── mirror.rs       // Helpers for mounting mirrored REST/SSE/WS routes
│   └── openapi.rs      // OpenClaw-specific OpenAPI export and security scheme
```

## 2. Security & Authentication Middleware

Security is critical since OpenClaw will have high-level permissions to modify system states and trigger sessions.

### 2.1 The `Auth` Extractor/Middleware

A dedicated middleware (or Axum Extractor) will guard the `/api/openclaw/v1/*` and `/api/openclaw/ws/*` namespaces.

1.  **Read Configuration**: Upon server startup, load the `OPENCLAW_API_KEY` from environment variables into application state. If `OPENCLAW_GATEWAY_ENABLED=false` or missing, the entire sub-router should reject all requests (`403 Forbidden`).
2.  **Inspect Header**: For every incoming request, inspect the `Authorization` header. It must conform to the `Bearer <token>` scheme.
3.  **Validate**: Compare the provided token with the loaded API key using a secure, constant-time equality check to prevent timing attacks.
4.  **Principal Mapping**: On success, inject a synthetic identity such as `OpenClaw Super Admin` into the request context. This identity is treated as system-admin-equivalent by RBAC checks.
5.  **Audit Tagging**: Attach metadata like `auth_source=openclaw`, request ID, and optional OpenClaw actor/session identifiers for observability.
6.  **Rejection**: If the token is missing, malformed, or incorrect, instantly abort the request with `401 Unauthorized`.

### 2.2 Super Admin Semantics

OpenClaw is intentionally modeled as a trusted automation operator, not as a normal project member:

*   It should be able to access the same internal APIs a system administrator can access.
*   Project-scoped permission checks should pass through the existing `system admin` branch rather than requiring endpoint-specific OpenClaw exceptions.
*   Any endpoint that remains blocked from OpenClaw must be blocked explicitly and documented as an exception, not as a side effect of ordinary RBAC.

## 3. Route Mirroring Strategy

### 3.1 REST and SSE Mirroring

The gateway must mirror the internal REST/SSE surface according to this rule:

*   Internal route: `/api/v1/<path>`
*   OpenClaw route: `/api/openclaw/v1/<path>`

The mirrored endpoint must preserve:

*   the same HTTP method
*   the same path and query parameters
*   the same request body schema
*   the same response body schema
*   the same business logic and status-code semantics

Examples:

*   `/api/v1/projects/:id` -> `/api/openclaw/v1/projects/:id`
*   `/api/v1/tasks/:task_id/attempts` -> `/api/openclaw/v1/tasks/:task_id/attempts`
*   `/api/v1/attempts/:id/stream` -> `/api/openclaw/v1/attempts/:id/stream`

### 3.2 WebSocket Mirroring

Root-level WebSocket routes should be mirrored under an OpenClaw namespace as well:

*   Internal route: `/ws/<path>`
*   OpenClaw route: `/api/openclaw/ws/<path>`

Examples:

*   `/ws/attempts/:id/logs` -> `/api/openclaw/ws/attempts/:id/logs`
*   `/ws/projects/:project_id/agents` -> `/api/openclaw/ws/projects/:project_id/agents`

### 3.3 Intentional Exceptions

Even with the goal of exposing the full internal admin surface, a few routes should remain outside the mirrored contract:

1.  **Gateway Bootstrap/Auth Endpoints**: `/auth/login`, `/auth/register`, refresh/logout flows are unnecessary because OpenClaw authenticates with its dedicated API key.
2.  **Browser Callback Endpoints**: Human-interactive OAuth callbacks (for example browser redirect handlers) remain browser-oriented and should not be re-modeled as OpenClaw admin tools.
3.  **Non-OpenAPI Scrape Endpoints**: Raw operational scrape endpoints such as Prometheus text metrics can be exposed separately, but they are not part of the main mirrored OpenAPI contract unless explicitly wrapped.

## 4. OpenAPI / Swagger Integration (`utoipa`)

To enable OpenClaw to automatically discover capabilities, we will leverage the `utoipa` crate. `utoipa` generates OpenAPI descriptions at compile time based on code annotations.

### 4.1 Setup `utoipa`
We must add `utoipa` and `utoipa-swagger-ui` to `crates/server/Cargo.toml`.

### 4.2 Defining the API Specification
We create an `OpenApi` struct specifically for the OpenClaw namespace. It should be generated from the same path/schema registrations as the internal API wherever possible, but with:

*   the OpenClaw base paths
*   the OpenClaw bearer security scheme
*   the mirrored route list for the full administrative surface

```rust
use utoipa::{OpenApi, Modify};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

#[derive(OpenApi)]
#[openapi(
    paths(
        // Register mirrored OpenClaw endpoints here
        crate::routes::openclaw::projects::get_projects,
        crate::routes::openclaw::tasks::create_task,
        crate::routes::openclaw::task_attempts::get_attempt,
        crate::routes::openclaw::settings::get_settings,
        // ...continue for the full mirrored admin surface
    ),
    components(
        schemas(Project, TaskResponse, SettingsResponse, AttemptResponse)
    ),
    modifiers(&SecurityAddon)
)]
pub struct OpenClawApiDoc;

// Define the Bearer Auth Scheme for Swagger UI / Spec
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "api_key_bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("API Key")
                    .build(),
            ),
        )
    }
}
```

### 4.3 Exposing the Definitions
Two routes will be exposed **unauthenticated** so tools can read the schema:
1.  `GET /api/openclaw/openapi.json`: Returns the raw OpenAPI v3 JSON spec payload.
2.  `GET /api/openclaw/swagger-ui`: (Optional but helpful for testing) Serves the visual Swagger UI.

The OpenClaw spec is expected to describe the mirrored admin/business API surface, not a tiny integration subset.
