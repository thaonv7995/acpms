---
name: deploy-ssh-remote
description: Build artifact and deploy directly to remote server via SSH.
---

# Deploy SSH Remote

## Objective
Build the artifact, then **deploy directly** to the configured SSH server. You SSH to the server, copy the artifact, and run the deploy—no API call.

## When This Applies
- Task type is **Deploy**
- Project has an SSH deployment environment configured (Project → Deployments → Environments)
- System has prepared deploy context in `.acpms/deploy/`

## Deploy Context (Prepared by System)
Before you run, the system writes to `.acpms/deploy/`:
- **ssh_key**: SSH private key (chmod 600)
- **config.json**: `{ "host", "port", "username", "deploy_path" }`

## Workflow

### 1. Build Artifact
- Run the project's build command (e.g. `npm run build`, `cargo build --release`, `make build`)
- Verify artifact output exists (e.g. `dist/`, `build/`, `target/release/`)
- Record artifact path(s)

### 2. Run Tests (Optional)
- If the project has tests, run them before deploy
- Do not proceed if critical tests fail

### 3. Deploy via SSH
- Read `.acpms/deploy/config.json` for host, port, username, deploy_path
- Use `ssh -i .acpms/deploy/ssh_key -o StrictHostKeyChecking=accept-new -p <port> <user>@<host>` to connect
- Copy artifact to remote: `rsync -avz -e "ssh -i .acpms/deploy/ssh_key -p <port>" <artifact_path>/ <user>@<host>:<deploy_path>/`
- Or use `scp -i .acpms/deploy/ssh_key -P <port> <artifact_path>/* <user>@<host>:<deploy_path>/`
- If the project has a deploy script on the server (e.g. `./deploy.sh`), SSH and run it after copying

### 4. Report Success
- Confirm deployment completed
- Include in final report: `build_status`, `artifact_paths`, `deployment_status`, `deploy_target`

## Decision Rules
| Situation | Action |
|-----------|--------|
| Build fails | Report failure, do not complete. Fix build issues first. |
| No build script found | Report blocked. Project needs build configuration. |
| `.acpms/deploy/` missing | Report blocked. No deploy context configured. |
| SSH connection fails | Report failure, include error. Check host/port/credentials. |
| Copy/deploy fails | Report failure, include error. |

## Output Contract
Include in final report:
- `build_status`: `success` or `failed`
- `artifact_paths`: list of produced artifact paths
- `deployment_status`: `success` or `failed`
- `deploy_target`: host and path (e.g. `user@host:deploy_path`)

## Notes
- You deploy **directly** via SSH. No API call needed.
- The SSH key is in `.acpms/deploy/ssh_key`—use it for all SSH/rsync/scp commands.
- Keep the key secure; do not log or expose it.
