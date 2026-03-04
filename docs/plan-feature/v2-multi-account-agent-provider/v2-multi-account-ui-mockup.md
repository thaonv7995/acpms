# Multi-Account (V2.0) UI Design Proposal

Based on the [settings-auth-ui-preview-v5-reauth-only.png] mockup, our current screen is divided into two panels:
- **Left Panel (PROVIDER):** Global default provider pills + 3 fixed rows (Codex, Claude, Gemini).
- **Right Panel (AUTH SESSION):** The active authentication session state.

To support Multi-Account (V2.0) while keeping this premium dual-pane UI intact, we will evolve the **Left Panel** into an **Expandable Provider List**.

## Proposed UI Concept (Left Panel)

### Section 1: Target Engine
This remains unchanged. The system orchestrator needs to know *which AI engine* to route tasks to.
```text
Default provider for new attempts
[ Codex CLI ]  [ Claude Code ]  [ Gemini CLI ]
```

### Section 2: Account Management (Accordion/Expandable List)
Instead of 1 row per provider, each provider row becomes an accordion header showing the summary of accounts. Clicking it expands the specific accounts.

```text
▼ Codex CLI (OpenAI)                           [ 2 Accounts ] [ default ] [ + Add ]
  ↳  Production Codex                      [ available ]     [ Re-auth ] [ 🗑 ]
  ↳  Fallback Codex                        [ expired ]       [ Re-auth ] [ 🗑 ]

▶ Claude Code                                  [ 1 Account ]              [ + Add ]

▼ Gemini CLI                                   [ 0 Accounts ]             [ + Add ]
  ↳  No accounts connected. Click [+ Add] to authenticate a new Gemini profile.
```

## User Flows

### Flow 1: Adding a New Account
1. User clicks the `[ + Add ]` button on the **Gemini CLI** header row.
2. A small inline prompt or modal asks for the `Profile Name` (e.g., "Team Gemini 3").
3. User confirms. The backend generates a `session_id` and a `profile_id`.
4. The **Right Panel (AUTH SESSION)** immediately lights up with the OOB/Loopback proxy instructions (Wait for URL, enter code), exactly matching the current V5 UI design!

### Flow 2: Re-Authenticating an Expired Account
1. User sees "Fallback Codex" is `[ expired ]`.
2. User clicks `[ Re-auth ]` explicitly on that sub-row.
3. The Right Panel updates with the Codex Device Flow (ABCD-1234) tied specifically to routing credentials back into "Fallback Codex"'s virtual directory.

## Why this design works perfectly with V5:
1. **Preserves the Right Panel:** The interactive Auth Session side-panel requires zero changes. It just listens to the `session_id` like it already does.
2. **Clean Layout:** By collapsing providers that aren't being edited, the UI avoids clutter.
3. **Clear Load-Balancing UX:** Grouping accounts *under* the Provider header makes it obvious to the admin that if they set "Codex" as the Default Provider, requests will be load-balanced across the 2 active nested accounts.

## Visual Mockup
Here is an AI-generated mockup visualizing this exact Accordion UI concept, proving it fits seamlessly into the existing dark theme:
<!-- Mockup image placeholder (add docs/todo-feat/multi_account_settings_ui.png if available) -->
