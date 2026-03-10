# UI Flow: Authentication & Settings

These screens cover user entry, identity management, and system configuration.

## 1. Login & Registration
- **Screen**: `LoginPage.tsx`
- **Action: Login**
    - **Trigger**: Click "Sign In" button.
    - **Submission**: Calls `POST /api/v1/auth/login`.
    - **Effect**: Stores JWT in local storage/cookies and navigates to `/dashboard`.
- **Action: Register**
    - **Trigger**: Switch to "Register" tab and click "Sign Up".
    - **Submission**: Calls `POST /api/v1/auth/register`.

## 2. User Settings
- **Screen**: `ProfilePage.tsx`
- **Action: Update Profile**
    - **Submission**: Calls `PUT /api/v1/users/:id`.
    - **Effect**: Updates name, email, or avatar.
- **Action: Change Password**
    - **Submission**: Calls `PUT /api/v1/users/:id/password`.

## 3. System Administration (Admin Only)
- **Screen**: `UserManagementPage.tsx`
- **Action: "Revoke Tokens" Button**
    - **Submission**: Calls `POST /api/v1/auth/revoke/:user_id`.
    - **Effect**: Forces a user to log out globally.
- **Action: Change Roles**
    - **Submission**: Calls `PUT /api/v1/users/:id` với `global_roles`.
    - **Backend**: `crates/server/src/routes/users.rs::update_user`.

## 4. Global System Settings
- **Action: Configure GitLab**
    - **Trigger**: Toggle "Edit" and click "Save".
    - **Submission**: Calls `PUT /api/v1/settings` with GitLab credentials.
    - **Encryption**: Tokens are encrypted at rest via `EncryptionService`.
- **Action: Configure Cloudflare**
    - **Trigger**: Toggle "Edit" and click "Save".
    - **Submission**: Calls `PUT /api/v1/settings` with Account ID, Token, and Zone info.
    - **Result**: Enables the [Deployment flow](../feature-flows/deployment-preview.md).
- **Action: Check Agent Status**
    - **Trigger**: Page load or manual "Refresh" click.
    - **Backend**: Calls `GET /api/v1/agent/status`.
    - **Verification**: Checks selected provider in `system_settings.agent_cli_provider` (canonical values only):
        - `claude-code`: `~/.claude` session exists and looks authenticated.
        - `openai-codex`: `codex --version` works (CLI installed).
        - `gemini-cli`: `gemini --version` works (CLI installed).
    - **Normalization**: Aliases `codex`→`openai-codex`, `gemini`→`gemini-cli` applied on save.

## 5. OpenClaw Access Management (Super Admin Only)
- **Screen**: `SettingsPage.tsx`
- **Entry Point**: A button such as `Manage OpenClaw` or `OpenClaw Access` in the super-admin settings area.
- **UI Pattern**: Clicking the button opens a modal or popup for OpenClaw management.

- **Action: View Enrolled OpenClaw Clients**
    - **Backend**: Calls `GET /api/v1/admin/openclaw/clients`.
    - **Effect**: Shows client name, `client_id`, status, enrolled time, last seen time, and key fingerprint summary.

- **Action: Generate New Bootstrap Prompt**
    - **Trigger**: Click `Add OpenClaw` or `Generate Bootstrap Prompt`.
    - **Submission**: Calls `POST /api/v1/admin/openclaw/bootstrap-tokens`.
    - **Effect**: Returns a single-use bootstrap token and a ready-to-send prompt that the admin can copy into another OpenClaw installation.
    - **Important**: The prompt should be shown only at creation time because the raw bootstrap token is not stored for later retrieval.

- **Action: Disable Client Access**
    - **Trigger**: Click `Disable Access` on a client row.
    - **Submission**: Calls `POST /api/v1/admin/openclaw/clients/{client_id}/disable`.
    - **Effect**: Blocks runtime auth for that OpenClaw client without deleting the enrollment record.

- **Action: Enable Client Access**
    - **Trigger**: Click `Enable Access` on a disabled client row.
    - **Submission**: Calls `POST /api/v1/admin/openclaw/clients/{client_id}/enable`.
    - **Effect**: Re-enables runtime auth for that OpenClaw client.

- **Action: Revoke Client**
    - **Trigger**: Click `Revoke Client` from row actions or details view.
    - **Submission**: Calls `POST /api/v1/admin/openclaw/clients/{client_id}/revoke`.
    - **Effect**: Permanently blocks the enrolled client and its keys. Reconnecting generally requires a new bootstrap flow.
