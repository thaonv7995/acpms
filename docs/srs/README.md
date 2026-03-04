# Software Requirements Specification (SRS) Index

This directory contains the detailed functional and non-functional requirements for each screen in the ACPMS platform. These documents define user actions, system inputs, business logic, and error handling.

## Core Screens
- [**Dashboard**](dashboard.md): System overview and real-time activity feeds.
- [**Projects Management**](projects.md): Repository connection and project lifecycle.
- [**Project Detail**](project-detail.md): Deep-dive into specific project health and resources.
- [**Deployment Environments**](deployment-environments.md): Configurable environment setup, deployment runs, logs, and rollback.
- [**Kanban Board**](kanban-board.md): Unified high-performance task management.
- [**Task Detail**](task-detail.md): Focused view for task metadata and agent execution.
- [**Agent Stream**](agent-stream.md): Global real-time log monitoring.
- [**Merge Requests**](merge-requests.md): Source control review and synchronization.
- **Requirement Management**: Requirements CRUD, AI-powered task breakdown, and status workflow (see UI: Requirements in project).

## System & Identity
- [**Authentication**](authentication.md): Identity management and access control.
- [**Settings & Integrations**](settings.md): Global configuration (GitLab, Cloudflare).
- [**Profile & Administration**](profile-admin.md): User settings and admin dashboard.

## Traceability
Each SRS action maps to a corresponding implementation in the [UI Interaction Flows](../ui-flows/README.md).
