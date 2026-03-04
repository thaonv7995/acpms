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
