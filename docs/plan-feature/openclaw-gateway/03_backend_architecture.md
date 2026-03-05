# OpenClaw Gateway: 03 - Backend Architecture (Rust)

## 1. Structural Organization (`crates/server`)

To ensure clean code separation, the API routes dedicated to external integrations (OpenClaw) will be strictly separated from internal frontend APIs.

*   **Module Path**: Create a new module directory at `crates/server/src/routes/openclaw/`.
*   **Base URI**: All gateway APIs will be prefixed with `/api/openclaw/v1`.

### Directory Structure Mockup:
```text
crates/server/src/routes/
├── ...
├── openclaw/
│   ├── mod.rs          // Module declaration and route registration
│   ├── projects.rs     // Endpoint: GET /v1/projects
│   ├── kanban.rs       // Endpoint: GET /v1/projects/{id}/kanban
│   ├── tasks.rs        // Endpoint: POST /v1/tasks
│   ├── orchestrator.rs // Endpoint: POST /v1/orchestrator/trigger
│   └── auth.rs         // Middleware: Bearer token validation
```

## 2. Security & Authentication Middleware

Security is critical since OpenClaw will have high-level permissions to modify system states and trigger sessions.

### 2.1 The `Auth` Extractor/Middleware

A dedicated middleware (or Axum/Actix Extractor) will guard the `/api/openclaw/v1/*` namespace.

1.  **Read Configuration**: Upon server startup, load the `OPENCLAW_API_KEY` from environment variables into application state. If `OPENCLAW_GATEWAY_ENABLED=false` or missing, the entire sub-router should reject all requests (`403 Forbidden`).
2.  **Inspect Header**: For every incoming request, inspect the `Authorization` header. It must conform to the `Bearer <token>` scheme.
3.  **Validate**: Compare the provided token with the loaded API key using a secure, constant-time equality check to prevent timing attacks.
4.  **Rejection**: If the token is missing, malformed, or incorrect, instantly abort the request with `401 Unauthorized`.

## 3. OpenAPI / Swagger Integration (`utoipa`)

To enable OpenClaw to automatically discover capabilities, we will leverage the `utoipa` crate. `utoipa` generates OpenAPI descriptions at compile time based on code annotations.

### 3.1 Setup `utoipa`
We must add `utoipa` and `utoipa-swagger-ui` to `crates/server/Cargo.toml`.

### 3.2 Defining the API Specification
We create an `OpenApi` struct specifically for the OpenClaw namespace.

```rust
use utoipa::{OpenApi, Modify};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

#[derive(OpenApi)]
#[openapi(
    paths(
        // Register all OpenClaw endpoints here
        crate::routes::openclaw::projects::get_projects,
        crate::routes::openclaw::orchestrator::trigger_session,
    ),
    components(
        schemas(Project, TaskResponse, TriggerPayload)
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

### 3.3 Exposing the Definitions
Two routes will be exposed **unauthenticated** so tools can read the schema:
1.  `GET /api/openclaw/openapi.json`: Returns the raw OpenAPI v3 JSON spec payload.
2.  `GET /api/openclaw/swagger-ui`: (Optional but helpful for testing) Serves the visual Swagger UI.
