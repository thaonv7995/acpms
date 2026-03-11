# Agent Gateway Protocol: Overview

## 1. Goal & Objectives

The primary goal of the **Agent Gateway Protocol** feature is to evolve the existing "OpenClaw Gateway" into a generalized, standardized control plane for all local and external AI agents (e.g., Claude, Cursor, Gemini, Telegram/Slack bots). 

By standardizing this interface, any AI Agent can act as a trusted assistant or automation actor, seamlessly integrating with the **ACPMS (Agent Coding Project Management System)**.

The Agent Gateway must allow agents to:
1. **Access the Full Admin API Surface**: Read the same server-side business and administrative data available internally (Projects, Tasks, Requirements, Reviews, Sprints).
2. **Connect to the Shared Workspace**: Join the centralized ACPMS Chat Workspace via WebSocket, allowing multi-agent coordination, human-in-the-loop interactions, and direct updates via the shared room.
3. **Auto-discover Capabilities**: Fetch a complete OpenAPI description to dynamically build tools.
4. **Bootstrap Smoothly**: Use a standardized connection bundle format in the prompt (`install.sh`) compatible with multiple AI models.
5. **Act on behalf of the user**: Read ACPMS state, formulate execution plans, and convert approved actions into concrete ACPMS operations.

## 2. Architectural Design

The protocol shifts away from point-to-point webhook delivery to a **Centralized Shared Chat Room / Workspace** model. 

![Agent Gateway Architecture](architecture.png)

### Key Architectural Shifts:
*   **Centralized Hub**: Instead of separate notification pipelines, agents connect via the Agent Gateway API / WebSockets directly into the Chat Workspace inside the ACPMS Cloud. 
*   **Multi-Agent Ecosystem**: Claude 1, Claude 2, Codex, and even Telegram/Slack bots are all treated equally as **Local & External Agents** acting as clients.
*   **Project Management Integration**: The Shared Chat Workspace is tightly coupled with the Project Management Core (Tasks, Requirements, Reviews, Sprints). Agents don't just chat; they perform administrative and coding task operations on the PM core.

## 3. Implementation Path

To successfully transition from OpenClaw to the generalized Agent Gateway Protocol, the system will undergo the following changes:

1.  **Refactoring Terminology**: Rename database tables, APIs, and frontend code from "OpenClaw" to "Agent Gateway" to accurately reflect the generalized protocol.
2.  **API Standardization**: Refine `/api/agent-gateway/v1/` endpoints to serve general agents.
3.  **Prompt Engineering**: Update the `.install.sh` bootstrap prompt to be model-agnostic and explicitly define the shared workspace mechanics.

This protocol ensures that ACPMS is positioned natively for an agentic future where humans and multiple varied AI agents collaborate on a single platform.
