---
name: deploy-cancel-stop-cleanup
description: Cancel deployment run, stop containers/processes on remote or local server, and clean up resources.
---

# Deploy Cancel, Stop & Cleanup

## Objective
Cancel an active deployment run, stop running containers or processes on the target server (remote SSH or local), and optionally remove/clean resources.

## When This Applies
- Task asks to cancel deploy, stop deployment, dừng container/process, xoá resource
- User wants to rollback or tear down a deployment

## Workflow

### 1. Cancel Deployment Run (ACPMS)
- User cancels via UI: Project → Deployments tab → chọn run đang chạy (queued/running) → nút **Cancel**
- Nếu task cung cấp run_id và API token: `curl -X POST "<API_URL>/api/v1/deployment-runs/<run_id>/cancel" -H "Authorization: Bearer <token>"`

### 2. Stop on Remote Server (SSH)
When deploy context exists (`.acpms/deploy/`):
- Read `.acpms/deploy/config.json` for host, port, username, deploy_path
- SSH: `ssh -i .acpms/deploy/ssh_key -o StrictHostKeyChecking=accept-new -p <port> <user>@<host>`

**Stop containers:**
- Docker Compose: `docker compose -f <path>/docker-compose.yml down` or `docker-compose down`
- Docker: `docker stop <container>` or `docker stop $(docker ps -q)`
- Docker Compose in deploy_path: `cd <deploy_path> && docker compose down`

**Stop processes:**
- `pkill -f <process_name>` (e.g. `pkill -f "node"`, `pkill -f "deploy.sh"`)
- `kill <pid>` if PID known
- `systemctl stop <service>` for systemd services

### 3. Stop on Local
- Same commands without SSH: `docker compose down`, `docker stop`, `pkill`, `systemctl stop`

### 4. Clean Up Resources
- Remove containers: `docker rm -f <container_id>`
- Remove images: `docker rmi <image>` (if safe)
- Remove volumes: `docker volume rm <name>` (caution: data loss)
- Remove deploy directory contents: `rm -rf <deploy_path>/*` (if task explicitly asks to remove)
- Prune: `docker system prune -f` (removes unused containers, networks)

### 5. Report
- List what was stopped/cancelled/removed
- Include `cleanup_status`: `success` or `failed`
- Include any errors

## Decision Rules
| Situation | Action |
|-----------|--------|
| No deploy context | Report blocked. Cannot SSH without .acpms/deploy/ |
| Task says "cancel only" | Cancel run via API/UI; do not stop containers |
| Task says "stop containers" | SSH and run docker compose down / docker stop |
| Task says "remove all" | Stop first, then remove with caution. Confirm scope. |
| Docker not found | Report: Docker not installed on target. |

## Safety
- Do **not** remove data volumes without explicit user confirmation
- Do **not** run `docker system prune -a` (removes all unused images) unless task says so
- Prefer `docker compose down` over `docker rm -f` for clean shutdown

## Output Contract
Include in final report:
- `cancel_status`: `success` or `skipped` or `failed`
- `containers_stopped`: list of stopped containers
- `processes_killed`: list of killed processes
- `resources_removed`: list of removed resources (if any)
- `cleanup_status`: `success` or `failed`
