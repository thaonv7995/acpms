# Feasibility Study: Multi-Account Provider Support

## 1. Executive Summary
**Status: Highly Feasible.**
The current architecture (spawning external CLI processes from a Rust backend) can be naturally extended to support multiple accounts for the same provider (e.g., 3 Gemini accounts) without requiring any changes to the official CLIs themselves. This provides a robust solution for **Load Balancing** and **Rate-Limit Bypass** for teams sharing the server.

## 2. Technical Mechanism: Virtual Configuration Environments
Most CLI tools determine where to save and load OAuth tokens based on standard OS environment variables. By manipulating these variables during the `Command::spawn()` phase, the Rust backend can trick the CLI into using isolated, virtual profiles.

### Provider Breakdown
1. **OpenAI Codex CLI / GitHub Copilot:**
   - Default Path: `~/.config/github-copilot/` or `~/.config/openai-codex/`
   - Injection Strategy: Override `HOME` and `XDG_CONFIG_HOME`.
   - Test Result: Successfully routes `credentials.json` to the mocked directory.
2. **Claude Code CLI:**
   - Default Path: `~/.claude/`
   - Injection Strategy: Override `CLAUDE_SESSION_DIR` or `HOME`.
3. **Gemini CLI:**
   - Default Path: `~/.gemini/`
   - Injection Strategy: Override `HOME` and `XDG_CONFIG_HOME`.

## 3. Proposed Architecture changes

### A. Database / State Management
Instead of a single global `gemini_is_authenticated` boolean in the backend state, introduce a `ProviderProfile` entity:
```rust
struct ProviderProfile {
    profile_id: String, // e.g., "gemini-prod-1"
    provider: String,   // "gemini-cli"
    status: AuthStatus, // Available, Expired
    config_dir: String, // e.g., "/app/data/profiles/gemini-prod-1"
}
```

### B. Auth Flow Modification (`/api/v1/agent/auth/initiate`)
When the Admin clicks "Add New Account", the backend generates a new `profile_id`, creates a physical directory, and spawns the auth process inside it:
```rust
let profile_dir = format!("/app/data/profiles/{}", profile_id);
std::fs::create_dir_all(&profile_dir)?;

Command::new("gemini")
    .arg("auth")
    .env("HOME", &profile_dir)
    .env("XDG_CONFIG_HOME", format!("{}/.config", profile_dir))
    .spawn()
```

### C. Execution Flow Modification (Load Balancing)
When a user requests an AI action, the Execution Orchestrator (`crates/executors/src/gemini.rs`) selects an active profile:
1. **Selection:** Backend randomly selects one of the `Available` profiles for the requested provider (Round-Robin or Random).
2. **Execution:** 
```rust
Command::new("gemini")
    .args(["-p", "--yolo", "--output-format", "stream-json"])
    .env("HOME", &selected_profile.config_dir) // Crucial!
    .spawn()
```

## 4. Impact Assessment
### Pros:
- **Rate-Limit Evasion:** A team of 10 developers can share 5 Gemini accounts, massively distributing the API quota and preventing IP/Account blockades.
- **Failover:** If one account's token expires, the backend can auto-route requests to the remaining healthy accounts while flagging the expired one for Admin re-auth.
- **Zero CLI Modification:** We don't need to fork or hack the proprietary CLI tools; we just manipulate their environment.

### Cons / Risks:
- **Disk Management:** We must ensure the `profiles/` directory is persistent across server restarts and secured with strict file permissions (`chmod 700`) so the system user running the Rust backend is the only one who can read the tokens.
- **UI Complexity:** The Settings Page will need a list view (Table) for accounts per provider instead of a single "Status" badge.

## 5. Conclusion
This feature is a major value-add for a centralized team server with low technical risk. It effectively turns the Agentic Coding Server into an intelligent Proxy/Load-Balancer for closed-source AI CLIs.
