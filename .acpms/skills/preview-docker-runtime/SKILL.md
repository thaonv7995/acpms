---
name: preview-docker-runtime
description: Use when a task needs a live preview URL for Web/API/Microservice and the runtime must be started, verified, restarted, or stopped via Docker container or Docker Compose instead of a host process.
---

# Preview Docker Runtime

## Objective
- Start preview from Docker, not from a host-level process.
- Keep preview controllable by ACPMS through `.acpms/preview-output.json`.
- Make follow-up preview updates predictable: stop old runtime, start new
  runtime, verify the URL, then update the contract.

## When This Applies
- User asks to start, deploy, rebuild, restart, or stop preview
- ACPMS asks the agent to produce `PREVIEW_TARGET`
- A web, API, or microservice task needs a local live URL for preview

## Inputs
- Current repo runtime files: `docker-compose.yml`, `compose.yml`, `Dockerfile`,
  framework manifests, and any existing build output
- Existing `.acpms/preview-output.json` if a preview already exists
- ACPMS env such as `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN`,
  `CLOUDFLARE_ZONE_ID`, and `CLOUDFLARE_BASE_DOMAIN` when public preview is expected
- Attempt/worktree context so container names and compose project names are unique

## Workflow
1. Inspect the repo for existing compose or Docker runtime files.
2. If an old preview exists, stop or replace it first.
3. Prefer a bind-mounted Docker workflow when code changes must reflect quickly.
4. Start the runtime for real:
   - `docker compose -p <project> up -d --build`, or
   - `docker run -d ...`
5. Verify the local runtime with a real HTTP check before claiming success.
6. Write `.acpms/preview-output.json` only after the HTTP check passes.
7. Emit `PREVIEW_TARGET` and `PREVIEW_URL` only after verification succeeds.

## Decision Rules
| Situation | Action |
|---|---|
| Repo already has a safe compose file | Reuse it. |
| Repo only has `Dockerfile` | Create a small compose wrapper if needed for controllable restarts. |
| Repo has neither compose nor `Dockerfile` | Create temporary preview runtime files under `.acpms/preview/`. |
| Old preview container exists | Stop or remove it before starting the new one. |
| Port conflict exists | Pick a new host port and update the contract. |
| Runtime files exist but no container is serving traffic | Start the runtime; do not report success from config validation alone. |
| Docker preview cannot be started | Emit `DEPLOYMENT_FAILURE_REASON: <root cause>`. |

## Output Contract
Write `.acpms/preview-output.json` with:
- `preview_target`: always the reachable local Docker URL
- `preview_url`: local URL for local-only preview, or public URL when a tunnel exists
- `runtime_control`: enough metadata to stop/restart the runtime later

Also emit:
- `PREVIEW_TARGET: http://127.0.0.1:<port>`
- `PREVIEW_URL: <public-or-local-url>`

## Guardrails
- Do not run preview as a bare host process like `npm run dev`, `vite preview`,
  `python app.py`, or `cargo run` directly in the worktree shell.
- Never claim preview is ready without a real HTTP check.
- Never confuse `docker compose config`, `docker build`, or file creation with a
  running preview.
- Never emit fake or placeholder URLs.

## Related Skills
- `setup-cloudflare-tunnel`
- `deploy-cancel-stop-cleanup`
- `deploy-precheck-cloudflare`
- `update-deployment-metadata`
