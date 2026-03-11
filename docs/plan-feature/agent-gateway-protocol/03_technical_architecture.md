# Agent Gateway Protocol: Technical Architecture

## 1. System Components Overview

The Agent Gateway Protocol relies on several foundational technical components to ensure secure, real-time, and bidirectional communication between external AI agents and the internal ACPMS ecosystem.

### 1.1 The Gateway API Layer
- **Role**: Replaces the old `openclaw` namespace with a generalized `/api/agent-gateway/v1/` namespace.
- **Function**: Exposes RESTful endpoints for CRUD operations on Tasks, Requirements, Sprints, and Executions.
- **Hybrid Assignment**: Tasks include an `assigned_to` field which can be an `AgentID` or a `HumanID`.
  - If `assigned_to` is a Human, the Gateway disables automated event triggers for that task (no agent spawning).
  - If `assigned_to` is an Agent, moving the task to `In Progress` triggers a WebSocket payload to the specific agent to begin autonomous execution.
- **Discovery**: Exposes an auto-generated `/api/agent-gateway/openapi.json` so any modern LLM can dynamically generate tool-calling structures without hardcoded SDKs.

### 1.2 The Shared Workspace (Web & CLI)
- **Role**: A continuous, bidirectional stream utilizing WebSocket and SSE.
- **Web Interface**: A modern Slack-like UI for management and human-in-the-loop interaction.
- **CLI Interface (Tmux)**: For local AI agents and human developers working in the terminal.
  - **Mechanic**: The Gateway bootstrap script (`install.sh`) includes a `chat-cli` utility.
  - **Layout**: Utilizes **Tmux** to create a vertical split. 
    - **Primary Pane**: The main coding/terminal workspace.
    - **Side Pane**: A real-time stream of the Shared Workspace using `acpms-chat-cli`, allowing the user to see agent deliberations alongside their code.
  - **Interaction**: The CLI tool allows sending messages directly to the gateway from the terminal (e.g., `acpms chat "I've updated the config, please re-test"`).

### 1.3 Role & Persona Engine
- **Role**: Manages the "Virtual Employee" profiles (**PO, PM, BA, DEV, QA**).
- **Function**: 
  - **Product Owner (PO)**: Focuses on "What" and "Why". Manages Requirements and value ranking.
  - **Project Manager (PM)**: Focuses on "How" and "When". Coordinates Tasks, Sprints, and unblocks rooms.
  - **BA/DEV/QA**: Execution roles focused on technical refinement, coding, and validation.
  - Every agent identifies its `AgentRole` during onboarding.
  - The Gateway enforces role-specific capabilities (e.g., only a PO/BA can approve a Requirement; only a PM can close a Sprint).
  - Maintains `AgentMetadata` (Avatar, Bio, Model Type) so humans can distinguish between different AI "employees".

### 1.4 Meeting Rooms & Shared Workspaces
- **Role**: Dynamic WebSocket rooms for ad-hoc collaboration.
- **Function**:
  - **The "Main Office"**: A default room for each project where all agents and humans see high-level updates.
  - **"Meeting Rooms"**: Temporary or persistent chat rooms created around specific Tasks or Requirements (e.g., "Refinement Meeting for Feature X").
  - **Threaded Discussions**: Support for threading so agents can deliberate over complex technical decisions without cluttering the main channel.

## 2. Technical Workflow (Lifecycle of an Agent)

1. **Bootstrap**: The human user generates a "Connection Prompt" from the ACPMS UI. The prompt contains the base URL, bootstrap token, and instructions.
2. **Setup Phase**: The agent reads the prompt, calls the `/api/agent-gateway/bootstrap/complete` endpoint with its public key and **Requested Role** (e.g., `role: "DEV"`), and receives a permanent Client ID.
3. **Context Loading**: The agent periodically calls the Gateway to fetch current active tasks and project documents relevant to its role.
4. **Subscription**: The agent connects to `/api/agent-gateway/ws/rooms/{room_id}` to join the project's shared workspace and any active meeting rooms.
5. **Collaboration**: Agents exchange messages in the Shared Workspace. A PM Agent might tag a DEV Agent: *"@DevAgent, please explain the delay on task #42"*.
6. **Execution**: Based on discussions or direct human requests, the agent executes actions via the Gateway API.
7. **Reporting**: The agent sends status updates back to the Shared Workspace.

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
