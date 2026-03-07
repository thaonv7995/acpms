---
name: preview-docker-runtime
description: Use when a task needs a live preview URL for Web/API/Microservice and the runtime must be started, verified, restarted, or stopped via Docker container or Docker Compose instead of a host process.
---

# Preview Docker Runtime

## Objective
- Start preview from Docker, not from a host-level process.
- Keep preview controllable by ACPMS through `.acpms/preview-output.json`.
- Make follow-up preview updates predictable: stop old runtime, start new runtime, verify URL, update contract.

## Non-Negotiable Rules
- Do **not** run preview as a bare host process like `npm run dev`, `vite preview`, `python app.py`, or `cargo run` directly in the worktree shell.
- Do **not** leave orphan preview processes outside Docker.
- Preview **must** run inside either:
  - `docker compose`
  - `docker run`
- Preview **must** publish machine-readable control metadata in `.acpms/preview-output.json`.

## When This Applies
- User asks to start, deploy, rebuild, restart, or stop preview.
- ACPMS asks the agent to produce `PREVIEW_TARGET`.
- A Web/API/Microservice task needs a local live URL for preview.

## Preferred Runtime Order
1. Existing repo `docker-compose.yml` / `compose.yml`
2. Existing repo `Dockerfile` + small compose wrapper
3. Temporary preview compose file under `.acpms/preview/`

## Start / Rebuild Workflow
1. Inspect repo for:
   - `docker-compose.yml`, `docker-compose.yaml`, `compose.yml`, `compose.yaml`
   - `Dockerfile`
   - framework/runtime hints (`package.json`, `requirements.txt`, `go.mod`, `Cargo.toml`)
2. If an old preview exists, read `.acpms/preview-output.json` and stop/remove the old runtime first.
3. Prefer a bind-mounted Docker workflow so code changes in the worktree are reflected in preview.
4. Start the preview runtime with a stable ACPMS-specific name:
   - container example: `acpms-preview-<attempt-id-short>`
   - compose project example: `acpms-preview-<attempt-id-short>`
5. Verify the app responds from the host:
   - `curl -I http://127.0.0.1:<port>`
   - or equivalent health/path check for the actual app
6. Write `.acpms/preview-output.json` before finishing.

## Stop Workflow
1. Read `.acpms/preview-output.json` if present.
2. If `runtime_control.runtime_type=docker_compose_project`:
   - stop with `docker compose -p <project> down --remove-orphans`
3. If `runtime_control.runtime_type=docker_container`:
   - stop with `docker rm -f <container_name>`
4. Verify the old preview URL no longer responds.
5. Update `.acpms/preview-output.json` to mark the runtime stopped or rewrite it with the new runtime after restart.

## Required File Contract
For local-only preview, write `.acpms/preview-output.json` with this shape:

```json
{
  "preview_target": "http://127.0.0.1:3000",
  "preview_url": "http://127.0.0.1:3000",
  "runtime_control": {
    "controllable": true,
    "runtime_type": "docker_compose_project",
    "compose_project_name": "acpms-preview-ab12cd34",
    "control_source": "preview-docker-runtime"
  }
}
```

When a public tunnel such as Cloudflare is also available, keep `preview_target`
as the local Docker URL and write the public address to `preview_url`:

```json
{
  "preview_target": "http://127.0.0.1:3000",
  "preview_url": "https://task-abc.trycloudflare.com",
  "runtime_control": {
    "controllable": true,
    "runtime_type": "docker_compose_project",
    "compose_project_name": "acpms-preview-ab12cd34",
    "control_source": "preview-docker-runtime"
  }
}
```

For a single container:

```json
{
  "preview_target": "http://127.0.0.1:3000",
  "preview_url": "http://127.0.0.1:3000",
  "runtime_control": {
    "controllable": true,
    "runtime_type": "docker_container",
    "container_name": "acpms-preview-ab12cd34",
    "control_source": "preview-docker-runtime"
  }
}
```

## Output Requirements
- Also print:
  - `PREVIEW_TARGET: http://127.0.0.1:<port>`
- Always print:
  - `PREVIEW_URL: <url>`
- If a public URL exists, `PREVIEW_URL` should be that public URL.
- If only a local preview exists, `PREVIEW_URL` should be the same local URL as `PREVIEW_TARGET`.

## Decision Rules
| Situation | Action |
|---|---|
| Repo already has compose | Reuse it if it can start a live preview safely |
| Repo has only Dockerfile | Create a small compose wrapper for stable restart/stop |
| Repo has neither compose nor Dockerfile | Create temporary `.acpms/preview/docker-compose.preview.yml` |
| Old preview container exists | Stop/remove it before starting new preview |
| Port conflict | Pick a new host port, then update `preview-output.json` |
| Docker build is slow but app can run with bind mount | Prefer bind mount dev/preview mode |
| Cannot start preview in Docker | Output `DEPLOYMENT_FAILURE_REASON: <root cause>` |

## Guardrails
- Never claim preview is ready without a real HTTP check.
- Never output fake or placeholder URLs.
- Never rely on worktree-local background processes outside Docker.
- Prefer names that are unique per attempt to avoid collision between follow-ups.
- When preview is rebuilt, overwrite old contract values with the new runtime metadata.

## Related Skills
- `setup-cloudflare-tunnel`: use after local Docker preview is reachable and ACPMS needs `PREVIEW_TARGET`
- `deploy-cancel-stop-cleanup`: use when cleanup/stop requires extra Docker shutdown steps
