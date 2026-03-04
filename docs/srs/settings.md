# SRS: Settings & Integrations

## 1. Introduction
The Settings screen provides global configuration for third-party integrations (GitLab and Cloudflare) and system-wide behavior.

## 2. Access Control
- **Roles**: Admins and Product Owners.
- **Permissions**:
  - `Admin`: Can modify all settings.
  - `PO`: Can view settings but require approval/admin role for modification of security tokens.

## 3. UI Components
- **Integrations Panel**: Config cards for GitLab (Instance URL, Token) and Cloudflare (Account ID, Zone, Domain).
- **Agent Runtime Panel**:
  - Provider selector (canonical values): `claude-code` | `openai-codex` | `gemini-cli`
  - Input aliases normalized on save: `codex` → `openai-codex`, `gemini` → `gemini-cli`
  - Optional API keys for Codex/Gemini (stored encrypted at rest)
  - Status indicator (Connected/Disconnected) via `GET /api/v1/agent/status`
- **System Logs**: View of global background job logs.

## 4. Functional Requirements

### [SRS-SET-001] Configure GitLab Integration
- **Trigger**: Edit and Save within "Source Control" card.
- **Input**: `instance_url`, `personal_access_token`.
- **Output**: Success toast; "Connected" status indicator.
- **System Logic**: Calls `PUT /api/v1/settings`. Backend encrypts the token before DB storage.
- **Validation**: Verifies token validity by calling GitLab `/user` endpoint immediately.

### [SRS-SET-002] Configure Cloudflare Deployment
- **Trigger**: Edit and Save within "Deployment" card.
- **Input**: `account_id`, `api_token`, `zone_id`.
- **Output**: Success toast.
- **System Logic**: Calls `PUT /api/v1/settings`.
- **Validation**: Verifies Cloudflare API Token permissions.

### [SRS-SET-003] Refresh Agent Provider Status
- **Trigger**: Manual Click "Refresh" or Page Mount.
- **Input**: None.
- **Output**: Status message (Connected/Disconnected) and session info.
- **System Logic**: Calls `GET /api/v1/agent/status`. Verifies the selected CLI provider is installed and authenticated.

### [SRS-SET-005] Configure Agent CLI Provider
- **Trigger**: Edit and Save within "Agent CLI Provider" card.
- **Input**: `agent_cli_provider` (canonical: `claude-code` | `openai-codex` | `gemini-cli`).
- **Output**: Success toast; provider status refresh reflects new selection.
- **System Logic**: Calls `PUT /api/v1/settings`. Backend normalizes aliases (`codex`→`openai-codex`, `gemini`→`gemini-cli`) before validation and storage.

### [SRS-SET-004] Toggle Global Features
- **Trigger**: Switch interaction in Settings.
- **Input**: Feature flag (e.g., `disable_ai_auto_start`).
- **Output**: Persistent change in platform behavior.
- **System Logic**: Updates `system_settings` table.

## 5. Non-Functional Requirements
- **Encryption**: Secrets must never be rendered in cleartext in the UI after saving. Input fields for tokens should use the "Password" type with a visibility toggle.
- **Audit Logging**: Any change to global settings must be logged with the user ID and timestamp in an audit table.
