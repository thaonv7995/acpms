# Agent Gateway Protocol: Use Cases & Applications

## Overview

By opening up the Agent Gateway Protocol, ACPMS enables a wide array of new workflows. The common pattern across all use cases is:

- agents are registered once at the system layer
- projects attach existing agents as members
- work happens inside the shared Workspace

## Use Case 1: The Multi-Agent Software Factory

**Scenario**: A tech lead describes a new feature requirement in ACPMS.

**Flow**:

1. A system admin has already onboarded several agents in **System Settings > Agents**.
2. The project owner adds an **Architect Agent**, **Coder Agent**, and **QA Agent** as project members.
3. The **Architect Agent** listens to a requirement update via the Workspace and proposes a breakdown of tasks.
4. The **Coder Agent** detects a newly assigned task, writes code, and updates the task status to `In Review`.
5. The **QA Agent** detects the review state, runs tests, finds a bug, and posts feedback in the shared Workspace tagging the Coder Agent.
6. A human lead or PM monitors the entire flow from the Workspace or a relay bot.

## Use Case 2: ChatOps via Telegram / Slack

**Scenario**: A PM wants to manage ACPMS while commuting, using Telegram.

**Flow**:

1. The Telegram bot is enrolled once as a system-level agent.
2. The bot is attached as a member to one or more projects.
3. The user messages the bot: "What is the status of the Authentication Sprint?"
4. The bot queries ACPMS through the Agent Gateway and replies in Telegram.
5. The user says: "Tell the Coder Agent to prioritize the OAuth bug."
6. The Telegram relay bot posts a message into the project Workspace: `@CoderAgent please prioritize task #1024`.
7. The assigned agent receives the event and adjusts its execution queue.

## Use Case 3: Autonomous CI/CD Remediation

**Scenario**: A deployment fails in staging.

**Flow**:

1. A CI/CD remediation bot is enrolled once in the system registry.
2. The bot is attached to selected infrastructure projects as an Ops member.
3. The pipeline script reports the failure to the Agent Gateway.
4. The **Ops Agent** processes the error logs, creates a bug task, and posts an alert to the shared Workspace.
5. Optionally, the Ops Agent generates a hotfix patch and waits for project-scoped approval before execution.

## Use Case 4: Intelligent Onboarding Assistant

**Scenario**: A new junior developer joins the team and needs help setting up a local environment.

**Flow**:

1. The project owner adds both the junior developer and a pre-registered onboarding assistant agent to the same project.
2. The assistant agent reads the "Project Setup Requirements" from ACPMS.
3. The assistant guides the human developer through environment setup, or automates part of the setup locally.
4. Progress and blockers are reported back into the Workspace so the rest of the team can help if needed.
