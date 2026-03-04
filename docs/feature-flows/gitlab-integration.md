# GitLab Integration & Webhooks Flow

This document describes how the system interacts with GitLab for project linking, code review, and automated synchronization.

## 1. Project Linking Flow

```mermaid
sequenceDiagram
    participant User
    participant Frontend
    participant Backend (GitLab API)
    participant GitLab

    User->>Frontend: Paste GitLab Project ID
    Frontend->>Backend (GitLab API): POST /api/v1/projects/:id/gitlab/link
    Backend (GitLab API)->>GitLab: Verify Project Access (PAT)
    Backend (GitLab API)->>GitLab: Create Webhook
    Backend (GitLab API)->>DB: Store GitLabConfiguration
    Backend (GitLab API)-->>Frontend: 200 OK
```

### Technical Details
- **Service**: `crates/services/src/gitlab.rs`
- **Credentials**: Personal Access Tokens (PAT) can be global (system settings) or per-project.
- **Webhook Registration**: Automatically registers a webhook in GitLab for `push` and `merge_request` events.

---

## 2. Automated Code Review (MR Sync)

```mermaid
sequenceDiagram
    participant GitLab
    participant WebhookHandler
    participant DB
    participant Frontend

    GitLab->>WebhookHandler: POST /api/v1/webhooks/gitlab (MR/Push Event)
    WebhookHandler->>WebhookHandler: Verify X-Gitlab-Token
    WebhookHandler->>DB: Queue webhook_events (async processing)
    WebhookHandler-->>GitLab: 200 Accepted (queued)
    Note over Frontend: UI refreshes MR data via task-level APIs
```

### Technical Details
- **Primary Handler**: `crates/server/src/routes/gitlab.rs::handle_webhook`.
- **Security**: Webhook payload được xác thực qua header `X-Gitlab-Token` đối chiếu `webhook_secret` theo project.
- **Persistence**: Event được queue vào webhook manager để xử lý bất đồng bộ.
- **Auto-Deploy on merge**: Luồng deploy riêng dùng `crates/server/src/routes/deployments.rs::handle_merge_webhook` tại endpoint `/api/v1/webhooks/gitlab/merge`.
