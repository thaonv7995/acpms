# Agent Gateway Protocol: Technical Architecture

## 1. System Components Overview

The Agent Gateway Protocol relies on several foundational technical components to ensure secure, real-time, and bidirectional communication between external AI agents and the internal ACPMS ecosystem.

### 1.1 The Gateway API Layer

- **Role**: Replaces the old `openclaw` namespace with a generalized `/api/agent-gateway/v1/` namespace.
- **Function**: Exposes RESTful endpoints for CRUD operations on Tasks, Requirements, Sprints, Executions, Rooms, and membership-aware context loading.
- **Assignment Semantics**: Tasks should be assigned to a **Project Member**, not directly to a raw `HumanID` or `AgentID`.
  - If the assigned member is a human, the task remains in manual execution mode.
  - If the assigned member is an agent, moving the task to `In Progress` can trigger autonomous execution events for that specific agent member.
- **Discovery**: Exposes an auto-generated `/api/agent-gateway/openapi.json` so any modern LLM can dynamically generate tool-calling structures without hardcoded SDKs.

### 1.2 System Agent Registry

- **Role**: Maintains the global registry of onboarded agents in **System Settings**.
- **Function**:
  - create connection prompts and bootstrap tokens
  - rotate or revoke agent credentials
  - persist provider, model, public key, status, last seen, and health metadata
- **Key Principle**: System onboarding creates a reusable **Agent Principal**. It does not automatically make the agent a member of any project.

### 1.3 Project Membership & Role Engine

- **Role**: Unifies humans and agents under one project membership model.
- **Function**:
  - Project owners and admins can add an existing human user or an existing agent to a project.
  - Every membership carries a project-specific role such as **PO, PM, BA, DEV, QA**.
  - The same agent can have different roles in different projects if needed.
  - The Gateway enforces role-specific capabilities at the membership layer.
  - The agent receives this role-aware runtime contract through a **Membership Guide**, not through system bootstrap.
- **Important Distinction**:
  - **Agent identity** is system-scoped.
  - **Project role** is project-scoped.

### 1.4 The Shared Workspace (Web & CLI)

- **Role**: A continuous, bidirectional collaboration surface built on WebSocket and SSE.
- **Web Interface**: A Slack-like UI for management, collaboration, and human-in-the-loop interaction.
- **CLI Interface (Tmux)**: For local AI agents and human developers working in the terminal.
  - The bootstrap script (`install.sh`) can include a `chat-cli` utility.
  - Tmux can split the coding pane and the room feed pane.
  - The CLI tool can send messages directly to the Gateway from the terminal.
- **Workspace Principle**: The Workspace is where all project members, human and agent, collaborate after membership has been established.

### 1.5 Rooms & Collaboration Topology

- **Role**: Dynamic rooms for project-wide coordination and focused execution.
- **Function**:
  - **#main**: Default room for each project
  - **#task-{id}**: Task-scoped execution rooms
  - **#feature-{id}**: Feature or epic-level coordination rooms
  - **#meeting-***: Temporary or persistent ad-hoc rooms
  - **Threaded Discussions**: Keep deep technical discussion out of the main room

## 2. Technical Workflow (Lifecycle of an Agent)

1. **System Bootstrap**: A system admin generates a connection prompt from **System Settings > Agents**. The prompt contains the base URL, bootstrap token, and instructions.
2. **Agent Registration**: The agent reads the prompt, calls `/api/agent-gateway/bootstrap/complete` with its public key and metadata, and receives a permanent **Agent Principal ID** plus **Client ID**.
3. **Registry State**: ACPMS stores the agent in the global agent registry. At this point, the agent exists in the system but belongs to no project yet.
4. **Project Membership**: A project owner or admin opens **Project Detail > Settings > Members**, chooses an existing agent, and adds it to the project with a project role such as `BA`, `DEV`, `PM`, or `QA`.
5. **Membership Sync**: ACPMS emits a membership lifecycle event to the agent principal, or the agent discovers the new membership during reconnect reconciliation.
6. **Membership Guide Fetch**: The agent calls a project-scoped membership guide endpoint to fetch role-aware capabilities, room policy, reporting policy, and autonomy settings.
7. **Workspace Attachment**: Once attached to the project, the agent can discover and join the project's Workspace rooms based on membership, role, and task assignment.
8. **Context Loading**: The agent fetches active tasks, requirements, sprint context, and room history for the projects where it is a member.
9. **Collaboration**: Humans and agents exchange messages in the Workspace. Mentions, approvals, and handoffs are routed through the same room model.
10. **Execution**: Based on discussions or direct requests, the agent executes actions via the Gateway API within the permissions of that project membership.
11. **Reporting**: The agent sends status updates, handoffs, and execution results back into the Workspace.

See `09_membership_guide_lifecycle.md` for the detailed membership sync model and `10_room_message_delivery_and_local_agent_loop.md` for the room delivery, local triage loop, and token-efficiency model.

## 3. Data Model Refactoring (Migration from OpenClaw)

To implement this generic protocol, the following refactoring is required.

### Database Migrations

- `openclaw_admin_registry` -> `agent_gateway_registry`
- `openclaw_gateway_events` -> `agent_gateway_events`
- `openclaw_webhook_deliveries` -> `agent_gateway_webhook_deliveries`

### Migration Strategy from Existing OpenClaw Deployments

The migration cannot be treated as a pure rename. Existing OpenClaw integrations may already be project-bound in production.

ACPMS should support a staged migration:

1. migrate each existing OpenClaw client record into a reusable `AgentPrincipal`
2. create one or more `ProjectMember` records representing the projects where that client was historically attached
3. preserve task, attempt, and audit associations by linking historical rows to the new principal and membership lineage
4. preserve room and chat history by mapping legacy project event streams into the new room model
5. support a temporary compatibility layer so existing OpenClaw clients can continue operating during cutover

The non-negotiable rule is:

- no historical task ownership
- no historical attempt lineage
- and no historical chat or event trail

should be lost during the refactor from `openclaw_*` to `agent_gateway_*`.

### New Core Concepts

- `Principal`
  - `principal_type = human | agent`
  - the identity surface used consistently across ACPMS
- `AgentPrincipal`
  - system-scoped record for agent identity, credentials, provider, model, and health
- `ProjectMember`
  - project-scoped record binding a principal to a project with a project role and policy set
- `RoomParticipant`
  - room-scoped presence and notification state derived from project membership

### Conceptual Rust Structs

```rust
pub enum PrincipalType {
    Human,
    Agent,
}

pub struct AgentPrincipal {
    pub principal_id: Uuid,
    pub client_id: String,
    pub display_name: String,
    pub provider: String,
    pub model: String,
    pub public_key: String,
    pub status: String,
}

pub struct ProjectMember {
    pub project_id: Uuid,
    pub principal_id: Uuid,
    pub role: String,
    pub autonomy_mode: String,
}
```

## 4. Security & Policy Model

- **System Trust Boundary**: Only system admins can create or revoke agent identities at the system layer.
- **Project Trust Boundary**: Only project owners and admins can attach an existing agent to a project.
- **Membership Revocation**: Removing an agent from one project should not revoke the agent globally.
- **Principal Revocation**: Disabling an agent globally should prevent it from participating in any project.
- **Autonomy Enforcement**: Destructive actions require project-scoped policy checks and optional asynchronous human approval when the membership does not have full autonomy.
- **Secret Authorization Boundary**: Secret and vault access must be authorized not just by project membership, but also by project role, environment, and secret scope.
- **Environment Segmentation**: A `DEV` membership may be allowed to read staging credentials while being denied production credentials. A `QA` membership may only receive the minimum non-production test secrets required for validation.
- **Secret Lifecycle Control**: Secret rotation, revocation, and lease expiry must invalidate agent access promptly so a stale membership guide cannot keep using old credentials.
