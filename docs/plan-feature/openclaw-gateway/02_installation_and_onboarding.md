# OpenClaw Gateway: 02 - Installation & Onboarding Experience

## 1. Seamless User Provisioning via `install.sh`

The gateway must be easy to set up for users who wish to integrate Agentic-Coding with OpenClaw. The provisioning process begins directly in the installation script (`install.sh`).

### 1.1 Interactive Prompt

During the execution of `install.sh`, after prompting for standard configurations (like public domain and admin credentials), the script will introduce a new interactive question:

```bash
Do you want to enable the OpenClaw Integration Gateway for external AI control? [y/N]
```

### 1.2 Automated Credential Generation

If the user answers `Y` (Yes), the script handles the complex part automatically:

1.  **Generate API Key**: Uses `openssl` or `/dev/urandom` to produce a secure, high-entropy string prepended with `oc_live_`. This key is used by OpenClaw to authenticate its API calls to Agentic-Coding.
2.  **Generate Webhook Secret (Optional)**: Produces another secure string prepended with `wh_sec_`. This secret is used only if the deployment enables optional ACPMS -> OpenClaw Webhook delivery. It is not required for the default outbound-only streaming integration model.

Because the gateway is intended to expose the full internal administrative API surface, the generated `OPENCLAW_API_KEY` must be treated as a **Super Admin secret**. In practical terms, possession of this key is equivalent to having root-level API control over the Agentic-Coding instance.

These credentials are then injected directly into the system's `.env` configuration file:

```env
# OpenClaw Gateway Integrations
OPENCLAW_GATEWAY_ENABLED=true
OPENCLAW_API_KEY=oc_live_5x8a9b2c3d4e5f6g7h8i9j0k
OPENCLAW_WEBHOOK_SECRET=wh_sec_a1b2c3d4e5f6g7h8i9j0k1l2
```

### 1.3 Success Report & Output

At the end of the installation process, the `print_success_report()` function should display:

1.  a connection-details section for operator reference
2.  a **ready-to-send bootstrap prompt** that the user can copy as one whole block and send directly to OpenClaw

The preferred onboarding flow is:

1.  copy the entire installer-generated OpenClaw prompt
2.  send it to OpenClaw
3.  let OpenClaw bootstrap itself by calling the ACPMS `Guide Endpoint`

The connection details still remain visible for debugging, auditing, and manual recovery.

```text
================================================================================
 OPENCLAW GATEWAY CONFIGURATION
================================================================================
 Copy the following details for reference or manual recovery:
 
 Base Endpoint URL : https://api.yourdomain.com/api/openclaw/v1
 OpenAPI (Swagger) : https://api.yourdomain.com/api/openclaw/openapi.json
 Guide Endpoint    : https://api.yourdomain.com/api/openclaw/guide-for-openclaw
 Global Event SSE  : https://api.yourdomain.com/api/openclaw/v1/events/stream
 API Key (Bearer)  : oc_live_5x8a9b2c3d4e5f6g7h8i9j0k
 Webhook Secret    : wh_sec_a1b2c3d4e5f6g7h8i9j0k1l2 (optional)
 Prompt File       : ~/.acpms/config/openclaw_bootstrap_prompt.txt
 
 Note: Keep these credentials secure. The API key grants Super Admin-equivalent access.
================================================================================

================================================================================
 OPENCLAW READY-TO-SEND PROMPT
================================================================================
 Copy everything below and send it to OpenClaw:

You are being connected to an ACPMS (Agentic Coding Project Management System) instance.

Your role for this ACPMS instance:
- act as a trusted Super Admin integration
- act as an operations assistant for the primary user
- load ACPMS context before making decisions
- analyze requirements using ACPMS data
- create/update ACPMS work only when requested or allowed by autonomy policy
- monitor running attempts and report meaningful updates to the user

ACPMS connection bundle:
- Base Endpoint URL: https://api.yourdomain.com/api/openclaw/v1
- OpenAPI (Swagger): https://api.yourdomain.com/api/openclaw/openapi.json
- Guide Endpoint: https://api.yourdomain.com/api/openclaw/guide-for-openclaw
- Global Event SSE: https://api.yourdomain.com/api/openclaw/v1/events/stream
- API Key (Bearer): oc_live_5x8a9b2c3d4e5f6g7h8i9j0k
- Webhook Secret: wh_sec_a1b2c3d4e5f6g7h8i9j0k1l2 (optional)

Your required first actions:
1. Store the API Key as the Bearer credential for ACPMS.
2. Call the Guide Endpoint first with `GET` for basic bootstrap and treat its response as the authoritative runtime guide.
3. Load the OpenAPI document.
4. Open and maintain the Global Event SSE connection.
5. Use only ACPMS OpenClaw routes:
   - /api/openclaw/v1/*
   - /api/openclaw/ws/*
6. Follow the ACPMS operating rules returned by the Guide Endpoint.

Bootstrap example (curl):
```bash
curl -sS \
  -X GET \
  -H "Authorization: Bearer oc_live_5x8a9b2c3d4e5f6g7h8i9j0k" \
  "https://api.yourdomain.com/api/openclaw/guide-for-openclaw"
```

Human reporting rules:
- report important status, analyses, plans, started attempts, completed attempts, failed attempts, blocked work, and approval requests
- do not expose secrets, API keys, or webhook secrets in user-facing output
- distinguish clearly between:
  - what ACPMS currently says
  - what you recommend
  - what you already changed

Do not ask the user to manually map these ACPMS credentials unless strictly necessary.
Use the Guide Endpoint to bootstrap yourself automatically.
================================================================================
```

The detailed specification for this prompt is defined in:

*   `docs/plan-feature/openclaw-gateway/12_installer_bootstrap_prompt.md`

Once OpenClaw receives that prompt, it should use the embedded values in this order:

1.  Store the `API Key (Bearer)` as the default authorization secret for ACPMS.
2.  Call the `Guide Endpoint` first to retrieve the instance-specific bootstrap instructions.
3.  Fetch the `OpenAPI (Swagger)` document to discover the mirrored ACPMS tool surface.
4.  Open the `Global Event SSE` connection and keep it active as the primary lifecycle/event transport.
5.  Store the `Webhook Secret` only if optional Webhook delivery is enabled for this deployment.

The installer prompt should make OpenClaw self-bootstrapping enough that the human user does not need to manually explain what ACPMS is or how OpenClaw should work with it.

## 2. Disabling or Resetting Credentials

If a user needs to disable the gateway or rotate their keys later:
*   They simply edit the `.env` file (located in `~/.acpms/config/.env` or `/etc/acpms/.env`) and modify the variables. 
*   Restart the systemd daemon (`sudo systemctl restart acpms-server`) or Docker container for changes to take effect.
*   Any rotation should be treated with the same urgency as rotating production administrator credentials.
