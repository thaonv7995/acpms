# Version 2.0: Multi-Account Architecture Deep Dive

## 1. Executive Summary
This document provides a detailed technical blueprint for transforming the Agentic Coding Server from a **Single-Provider System** (where one CLI credential governs the entire server) into a **Multi-Account Load Balanced System** (where admins can authenticate multiple profiles per provider, and the backend orchestrator routes requests across them).

## 2. System State Comparison

| Component | Current State (v1) | Target State (v2.0) |
| :--- | :--- | :--- |
| **Data Model** | `SystemSettings` has a single `agent_cli_provider` string field. | New `agent_profiles` DB Table. `SystemSettings` maintains the *default* provider type. |
| **Settings UI** | Dropdown to select Provider. Single "Sign In" button. | List/Table of active accounts per provider. "Add Account" button. |
| **Auth Flow** | Spawns CLI using default OS environment. | Spawns CLI using restricted `HOME` variables tied to the Profile ID. |
| **Orchestrator** | Executes `Command::new("gemini")` globally. | Queries DB for an active profile, injects `env("HOME", profile.dir)` randomly. |

## 3. Database Schema Changes

We will introduce a new table to track virtual profiles rather than storing credentials (which remain on disk managed by the CLI).

```sql
-- Migration: Create agent profiles table
CREATE TABLE IF NOT EXISTS agent_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL, -- 'gemini-cli', 'claude-code', 'openai-codex'
    profile_name TEXT NOT NULL, -- e.g., "Company Gemini 1"
    status TEXT NOT NULL DEFAULT 'disconnected', -- 'available', 'disconnected', 'expired'
    config_dir TEXT NOT NULL UNIQUE, -- e.g., "/app/data/agent_profiles/uuid/"
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_agent_profiles_provider ON agent_profiles(provider);
```

## 4. Backend Architecture (Rust)

### A. Authentication Injection (`crates/server/src/auth_runner.rs`)
When the Admin clicks "Authenticate" on the UI to add a new profile, the backend generates a `profile_id`, creates the DB record, creates a physical directory, and initiates the Flow:

```rust
let profile_base_dir = format!("/app/data/agent_profiles/{}", profile_id);
std::fs::create_dir_all(&profile_base_dir)?;

// Inject Virtual Environment for Authentication
Command::new("gemini")
    .arg("auth")
    .env("HOME", &profile_base_dir)
    .env("XDG_CONFIG_HOME", format!("{}/.config", profile_base_dir))
    .env("CLAUDE_SESSION_DIR", &profile_base_dir) // Cover all bases
    .spawn()
```

### B. Orchestrator Load Balancing (`crates/executors/src/orchestrator.rs`)
Before spawning an agent task, the orchestrator queries the DB for all `Available` profiles matching the selected default provider. It selects one using Round-Robin or Random selection to distribute the API rate limiting.

```rust
// 1. Fetch Setting (e.g. system is set to use 'gemini-cli')
let provider = system_settings.agent_cli_provider;

// 2. Load Balance Query
let profiles = sqlx::query!("SELECT * FROM agent_profiles WHERE provider = $1 AND status = 'available'", provider)
    .fetch_all(&db)
    .await?;

let selected = profiles.choose(&mut rand::thread_rng()).expect("No available profiles!");

// 3. Spawns Agent in Virtual Environment
Command::new("gemini")
    .args(["-p", "--yolo", "--output-format", "stream-json"])
    .env("HOME", &selected.config_dir) // Routes traffic via Account N
    .env("XDG_CONFIG_HOME", format!("{}/.config", selected.config_dir))
    .spawn()
```

## 5. UI Transformation (`frontend/src/pages/SettingsPage.tsx`)

### The "Agent Execution Runtime" Block
Currently, the UI shows:
> **Provider:** [Dropdown]  
> **Status:** [ Badge ] [Sign In]

In V2.0, this converts to a Hub:
1. **Global Default Provider:** A generic dropdown defining which AI (Claude/Gemini/Codex) the system defaults to.
2. **Account Pool Table:**
   - A data table listing `Profile Name | Provider | Status | Last Used | Actions (Re-Auth / Delete)`
   - An `[+ Add Account]` button. Clicking this opens a modal: "Choose Provider (Codex/Gemini/Claude) and Name this Profile".
   - The Auth Sequence (Device Flow / Loopback Proxy) operates exactly as defined in V1 but sends requests to `POST /api/v1/agent/profiles/{id}/auth/initiate`.

## 6. Migration & Rollout Strategy
1. **Phase 1 (Data Layer):** Implement the `agent_profiles` SQL migration and the CRUD endpoints.
2. **Phase 2 (Executor Injection):** Update `Command::new` in the executor crates to accept and enforce a `config_dir` override.
3. **Phase 3 (Legacy Wipe):** Deprecate the global `agent::check_provider_status` and transition entirely to querying the `agent_profiles` table to see if any nodes are `Available`.
4. **Phase 4 (UI Overhaul):** Replace the single Settings block with the Account Pool Table.

## Visual Mockup
Here is an AI-generated mockup visualizing this exact Accordion UI concept, proving it fits seamlessly into the existing dark theme:
<!-- Mockup image placeholder (add docs/todo-feat/multi_account_settings_ui.png if available) -->

## 7. Delta Analysis vs V1 Implementation (Current State)
Having reviewed the current V1 implementation (`crates/server/src/routes/agent.rs` and `services/agent_auth.rs`), here are the exact modifications needed to reach V2:

### A. Auth Initiation (`initiate_agent_auth`)
- **Current (V1):** `InitiateAgentAuthRequest` takes just `provider`. `launch_auth_process` spawns CLI cleanly.
- **V2 Delta:** `InitiateAgentAuthRequest` must accept an optional `profile_id`. If omitted, it creates a new one. `launch_auth_process` must retrieve this `profile_id` to build `profile_dir` and inject `env("HOME", profile_dir)`.

### B. Provider Probing (`check_provider_status`)
- **Current (V1):** Runs `codex login status` globally.
- **V2 Delta:** Must accept a `config_dir: &str` parameter so it can probe individual profiles independently:
  ```rust
  Command::new("gemini")
      .args(["-p", "ping"])
      .env("HOME", config_dir)
      // ...
  ```

### C. Session Model (`AuthSessionRecord`)
- **Current (V1):** No linkage to a specific profile.
- **V2 Delta:** Add a `profile_id: Option<Uuid>` directly to the `AuthSessionRecord` inside `agent_auth.rs` so that the backend correctly records the final "Available" state back to the specific Profile Database Row upon `AuthSessionStatus::Succeeded`.

### D. Settings Endpoints
- **Current (V1):** `GET /api/v1/agent/providers/status` returns an array of the global provider states.
- **V2 Delta:** We need a new CRUD router for `profiles` (e.g., `GET /api/v1/agent/profiles`) that returns the list of all configured profiles + their individual probed status.

### E. Task Executor (Orchestrator)
- **Current (V1):** In `crates/executors/src/orchestrator.rs` and `init_flow.rs`, the orchestrator calls `spawn_session` (e.g., `self.codex_client.spawn_session`) passing `provider_env` which might contain global API keys or empty vectors.
- **V2 Delta:** Before calling `spawn_session`, the orchestrator must:
  1. Fetch `agent_profiles` from the DB for the chosen Engine.
  2. Apply Load Balancing logic: Pick an `Available` profile.
  3. Inject `HOME`, `XDG_CONFIG_HOME`, and `CLAUDE_SESSION_DIR` mapped to the `profile_id`'s virtual directory straight into the `provider_env` parameter. The underlying executor libraries (`codex.rs`, `gemini.rs`, `claude.rs`) already pipe `provider_env` into `cmd.env(k, v)`, making this an incredibly clean integration!
