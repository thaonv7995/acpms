# Planned features

Features **planned for future implementation**. These docs describe design direction, feasibility studies, and implementation plans.

**Contributors welcome.** If you want to implement one of these features, please open an issue or PR to discuss with the maintainers first.

Features are grouped by folder below.

---

## [v2-multi-account](v2-multi-account-agent-provider/)

Multi-account / multi-tenant: architecture, UI mockup, legacy migration plan, feasibility study.

- [v2-multi-account-architecture.md](v2-multi-account/v2-multi-account-architecture.md) – Architecture
- [v2-multi-account-ui-mockup.md](v2-multi-account/v2-multi-account-ui-mockup.md) – UI mockup
- [v2-legacy-features-migration-plan.md](v2-multi-account/v2-legacy-features-migration-plan.md) – Legacy features migration plan
- [feasibility_study_multi_account.md](v2-multi-account/feasibility_study_multi_account.md) – Feasibility study

## [ephemeral-sandbox](ephemeral-sandbox/)

Docker container isolation for AI agent execution to ensure system security and dependency isolation.

- [01_overview.md](ephemeral-sandbox/01_overview.md) - Architecture & Overview
- [02_execution_flow.md](ephemeral-sandbox/02_execution_flow.md) - Docker Volume Mount Execution Flow

## [global-knowledge-base](global-knowledge-base/)

A retrieval-augmented generation (RAG) system for AI agents to pull community "SKILLs", coding conventions, and official documentation from GitHub and the internet.

- [01_concept.md](global-knowledge-base/01_concept.md) - The Global Knowledge Base vision
- [02_retrieval_architecture.md](global-knowledge-base/02_retrieval_architecture.md) - Architecture for ingesting and retrieving SKILLs

## [openclaw-gateway](openclaw-gateway/)

External OpenClaw integration as a Super Admin control plane for ACPMS, including mirrored APIs, bootstrap guidance, stream-first eventing, and optional Webhook support.

- [01_overview.md](openclaw-gateway/01_overview.md) - Overview and architecture
- [02_installation_and_onboarding.md](openclaw-gateway/02_installation_and_onboarding.md) - Installer and onboarding flow
- [03_backend_architecture.md](openclaw-gateway/03_backend_architecture.md) - Rust backend architecture
- [04_api_specifications.md](openclaw-gateway/04_api_specifications.md) - Mirrored API and gateway endpoints
- [05_webhook_engine.md](openclaw-gateway/05_webhook_engine.md) - Optional Webhook transport
- [06_api_design_standards.md](openclaw-gateway/06_api_design_standards.md) - Response and error standards
- [07_streaming_api.md](openclaw-gateway/07_streaming_api.md) - Global and attempt-specific SSE streams
- [08_hitl_api.md](openclaw-gateway/08_hitl_api.md) - Human-in-the-loop handling
- [09_bootstrap_guide_api.md](openclaw-gateway/09_bootstrap_guide_api.md) - Bootstrap guide endpoint
- [10_implementation_checklist.md](openclaw-gateway/10_implementation_checklist.md) - Implementation checklist for event stream and replay
- [11_operating_rules.md](openclaw-gateway/11_operating_rules.md) - Operating rules for command handling and user reporting
