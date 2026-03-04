# Build, Preview & Production Deployment Flow

This document describes how the system handles building projects, creating ephemeral preview environments, and deploying to production.

> Status ordering note: deployment/preview metadata finalization now runs **before**
> an attempt is marked `success`. The server no longer relies on a post-success
> event listener for this pipeline.

## 1. Build & Artifact Flow

```mermaid
sequenceDiagram
    participant User
    participant Backend (API)
    participant BuildService
    participant Worktree
    participant S3 (MinIO)

    User->>Backend (API): POST /api/v1/attempts/:id/build
    Backend (API)-->>User: 202 Accepted (Build started)
    Backend (API)->>BuildService: tokio::spawn(run_build(...))
    BuildService->>Worktree: npm install && npm run build
    BuildService->>BuildService: Create tar.gz of output dir
    BuildService->>S3 (MinIO): Upload tar.gz (presigned URL)
    BuildService->>DB: Record BuildArtifact
    Note over User: Poll GET /attempts/:id/artifacts for results
```

### Technical Details
- **Build Service**: `crates/services/src/build-service.rs`
- **Output Storage**: Artifacts are stored in S3/MinIO under `builds/{project_id}/{attempt_id}/artifacts.tar.gz`.
- **Auto-Detection**: The build command is auto-detected based on `ProjectType` (e.g., `npm run build` for Web, `cargo build --release` for API).

---

## 2. Ephemeral Preview Flow

```mermaid
sequenceDiagram
    participant User
    participant Backend (API)
    participant PreviewManager
    participant Cloudflare
    participant DNS

    User->>Backend (API): POST /api/v1/attempts/:id/preview
    Backend (API)->>PreviewManager: create_preview(attempt_id)
    PreviewManager->>Cloudflare: Create Tunnel
    PreviewManager->>Cloudflare: Create DNS CNAME record
    PreviewManager->>DB: Store Tunnel Credentials (encrypted)
    Backend (API)-->>User: 201 Created (Preview URL)
```

### Technical Details
- **Preview Manager**: `crates/preview/src/manager.rs`
- **Networking**: Uses Cloudflare Tunnels (Argo) to expose internal processes without open ports.
- **Dynamic Subdomains**: Generates URLs like `https://task-{attempt_id}.yourdomain.com`.

---

## 3. Production Deployment Flow

```mermaid
sequenceDiagram
    participant User
    participant Backend (API)
    participant DeployService
    participant Cloudflare/Container
    participant GitLab (Webhook)

    User->>Backend (API): POST /api/v1/projects/:id/deploy
    Backend (API)->>DeployService: deploy(project, artifact)
    
    alt Web (Cloudflare Pages)
        DeployService->>Cloudflare: Deploy to Pages
    else API (Cloudflare Workers)
        DeployService->>Cloudflare: Deploy to Workers
    else Microservice (Docker)
        DeployService->>Container: Push to Registry
    end
    
    DeployService->>DB: Record ProductionDeployment
    Backend (API)-->>User: 200 OK
```

### Technical Details
- **Deployment Service**: `crates/services/src/production-deploy-service.rs`
- **Auto-Deploy**: Can be triggered via GitLab Merge Webhook (`POST /api/v1/webhooks/gitlab/merge`).
- **Rollback**: Supported by superseding the active deployment record in the database.

---

## 4. Agent Structured Output Ingestion

Before an attempt is marked success, the executor parses structured fields from agent logs and persists them into `task_attempts.metadata`.

### Parsed Fields (from final report / skill outputs)
- `PREVIEW_TARGET` -> `preview_target`
- `PREVIEW_URL` -> `preview_url_agent`
- `deployment_status`
- `deployment_error`
- `deployment_kind`
- `production_deployment_status`
- `production_deployment_error`
- `production_deployment_url`
- `production_deployment_type`
- `production_deployment_id`
- `deploy_precheck`
- `deploy_precheck_reason`
- `smoke_status`
- `rollback_recommended`
- `delivery_status`

### Metadata Behavior
- Parsed values are stored under top-level keys where relevant.
- Full parsed deployment/report payload is also stored as `deployment_report`.
- This allows the pre-success deploy hook to detect agent-reported deploy outcomes and avoid duplicate backend deploy execution when appropriate.
