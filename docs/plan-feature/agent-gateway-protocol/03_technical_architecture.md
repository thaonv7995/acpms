# Agent Gateway Protocol: Technical Architecture

## 1. System Components Overview

The Agent Gateway Protocol relies on several foundational technical components to ensure secure, real-time, and bidirectional communication between external AI agents and the internal ACPMS ecosystem.

### 1.1 The Gateway API Layer
- **Role**: Replaces the old `openclaw` namespace with a generalized `/api/agent-gateway/v1/` namespace.
- **Function**: Exposes RESTful endpoints for CRUD operations on Tasks, Requirements, Sprints, and Executions.
- **Discovery**: Exposes an auto-generated `/api/agent-gateway/openapi.json` so any modern LLM can dynamically generate tool-calling structures without hardcoded SDKs.

### 1.2 The Shared WebSocket Workspace
- **Role**: A continuous, bidirectional stream utilizing WebSocket and SSE (Server-Sent Events).
- **Function**: 
  - Allows multiple agents (e.g., Claude, Codex, Telegram Bot) to join a specific "Room" associated with a Workspace or Project.
  - Broadcasts lifecycle events (e.g., `task_updated`, `attempt_failed`) in real-time so agents can react immediately without polling.
  - Enables Agent-to-Agent and Agent-to-Human chat messages.

### 1.3 Identity and Auth Engine
- **Role**: Manages API keys, bootstrap tokens, and asymmetric client proofs.
- **Function**: 
  - Every agent generates a local Ed25519 keypair and enrolls via a single-use bootstrap token.
  - The Gateway issues a dedicated `AgentClientIdentity` that maps to specific RBAC permissions, preventing external agents from performing unauthorized destructive actions.

## 2. Technical Workflow (Lifecycle of an Agent)

1. **Bootstrap**: The human user generates a "Connection Prompt" from the ACPMS UI. The prompt contains the base URL, bootstrap token, and instructions.
2. **Setup Phase**: The agent reads the prompt, calls the `/api/agent-gateway/bootstrap/complete` endpoint with its public key, and receives a permanent Client ID.
3. **Context Loading**: The agent periodically calls the Gateway to fetch current active tasks and project documents.
4. **Subscription**: The agent connects to `/api/agent-gateway/ws/events` to listen for real-time changes.
5. **Execution**: Based on a human request (via Telegram or local chat), the agent executes an action (e.g., `POST /api/agent-gateway/v1/tasks`).
6. **Reporting**: The agent sends a status update back to the Shared Workspace or directly to a notification channel via the Gateway API.

## 3. Data Model Refactoring (Migration from OpenClaw)

To implement this generic protocol, the following deep refactoring is required:

### Database Migrations
- `openclaw_admin_registry` ➔ `agent_gateway_registry`
- `openclaw_gateway_events` ➔ `agent_gateway_events`
- `openclaw_webhook_deliveries` ➔ `agent_gateway_webhook_deliveries`

### Core Structs (Rust)
```rust
// Old OpenClaw Struct
pub struct OpenClawGuideResponse { ... }

// New Generalized Struct
pub struct AgentGatewayGuideResponse {
    pub instruction_prompt: String,
    pub core_missions: Vec<String>,
    pub acpms_profile: AgentGatewayAcpmsProfile,
    pub operating_rules: AgentGatewayOperatingRules,
    // ...
}
```

## 4. Security & Rate Limiting
- **HMAC Signatures**: For optional outbound webhooks to legacy bots, ACPMS signs payloads using HMAC-SHA256 (`X-Agentic-Signature`).
- **Autonomy Mode Enforcement**: The backend maintains an `autonomy_mode` flag for each agent. Destructive actions (DELETE, heavy schema changes) require asynchronous Human-In-The-Loop approval via the Gateway if the agent does not possess `FULL_AUTONOMY`.
