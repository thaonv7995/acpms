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

At the end of the installation process, the `print_success_report()` function will display a dedicated, highly visible section containing the necessary connection details. This is designed for simple "copy and paste" by the user into the OpenClaw dashboard.

```text
================================================================================
 OPENCLAW GATEWAY CONFIGURATION
================================================================================
 Copy the following credentials into your OpenClaw system settings:
 
 Base Endpoint URL : https://api.yourdomain.com/api/openclaw/v1
 OpenAPI (Swagger) : https://api.yourdomain.com/api/openclaw/openapi.json
 Guide Endpoint    : https://api.yourdomain.com/api/openclaw/guide-for-openclaw
 Global Event SSE  : https://api.yourdomain.com/api/openclaw/v1/events/stream
 API Key (Bearer)  : oc_live_5x8a9b2c3d4e5f6g7h8i9j0k
 Webhook Secret    : wh_sec_a1b2c3d4e5f6g7h8i9j0k1l2 (optional)
 
 Note: Keep these credentials secure. The API key grants Super Admin-equivalent access.
================================================================================
```

OpenClaw should use these values in this order:

1.  Store the `API Key (Bearer)` as the default authorization secret for ACPMS.
2.  Call the `Guide Endpoint` first to retrieve the instance-specific bootstrap instructions.
3.  Fetch the `OpenAPI (Swagger)` document to discover the mirrored ACPMS tool surface.
4.  Open the `Global Event SSE` connection and keep it active as the primary lifecycle/event transport.
5.  Store the `Webhook Secret` only if optional Webhook delivery is enabled for this deployment.

## 2. Disabling or Resetting Credentials

If a user needs to disable the gateway or rotate their keys later:
*   They simply edit the `.env` file (located in `~/.acpms/config/.env` or `/etc/acpms/.env`) and modify the variables. 
*   Restart the systemd daemon (`sudo systemctl restart acpms-server`) or Docker container for changes to take effect.
*   Any rotation should be treated with the same urgency as rotating production administrator credentials.
