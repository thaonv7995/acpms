# Deployments API

API endpoints cho build và deployment.

## Base Path

`/api/v1/attempts/:id` và `/api/v1/projects/:id/deploy`

## Authentication

Hiện tại deployment handlers chưa enforce JWT ở route level.

---

## Endpoints

### 1. POST `/api/v1/attempts/:id/build`

Trigger build cho attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "build_command": "npm run build",
  "output_dir": "dist"
}
```

**Fields** (all optional):
- `build_command` (string): Build command
- `output_dir` (string): Output directory

#### Response

**Status**: `202 Accepted`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Build started successfully",
  "data": {
    "attempt_id": "...",
    "status": "building"
  }
}
```

Build runs in background. Poll `GET /api/v1/attempts/:id/artifacts` for results.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/deployments.rs::trigger_build`

---

### 2. GET `/api/v1/attempts/:id/artifacts`

Lấy danh sách build artifacts.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Artifacts retrieved successfully",
  "data": [
    {
      "id": "...",
      "attempt_id": "...",
      "artifact_path": "...",
      "size": 1024,
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/deployments.rs::list_artifacts`

---

### 3. GET `/api/v1/attempts/:id/preview`

Lấy preview URL (alias cho preview endpoint).

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Preview URL retrieved successfully",
  "data": {
    "url": "https://preview-xxx.cloudflare.com"
  }
}
```

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/deployments.rs::get_preview_url`

---

### 4. POST `/api/v1/projects/:id/deploy`

Trigger production deployment.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Request Body

```json
{
  "deployment_type": "cloudflare_pages",
  "build_artifact_id": "..."
}
```

**Fields**:
- `deployment_type` (string, required): "cloudflare_pages" | "cloudflare_workers" | "container"
- `build_artifact_id` (UUID, optional): Build artifact ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Deployment started successfully",
  "data": {
    "deployment_id": "...",
    "status": "deploying",
    "url": "https://..."
  }
}
```

**Implementation Note**: Permission checks cho endpoint này chưa được enforce trực tiếp trong handler hiện tại.

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page

**Backend**: `crates/server/src/routes/deployments.rs::trigger_deploy`

---

### 5. GET `/api/v1/projects/:id/deployments`

Lấy danh sách deployments của project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Deployments retrieved successfully",
  "data": [
    {
      "id": "...",
      "project_id": "...",
      "status": "success",
      "url": "https://...",
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page

**Backend**: `crates/server/src/routes/deployments.rs::list_deployments`

---

### 6. GET `/api/v1/deployments/:id`

Lấy thông tin deployment theo ID.

#### Path Parameters

- `id` (UUID, required): Deployment ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Deployment retrieved successfully",
  "data": {
    "deployment_id": "...",
    "deployment_type": "cloudflare_pages",
    "status": "success",
    "url": "https://..."
  }
}
```

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page

**Backend**: `crates/server/src/routes/deployments.rs::get_deployment`

---

### 7. POST `/api/v1/webhooks/gitlab/merge`

GitLab merge webhook (auto-deploy).

#### Request

**Headers**:
```
X-Gitlab-Token: <webhook_secret>
```

**Body**: GitLab merge event payload

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Webhook processed"
}
```

**Note**: 
- Tự động trigger deployment khi merge request được merged
- Chỉ deploy nếu project có auto-deploy enabled

#### Frontend Usage

Không sử dụng trực tiếp (GitLab gọi endpoint này)

**Backend**: `crates/server/src/routes/deployments.rs::handle_merge_webhook`

---

## Deployment Types

- **cloudflare_pages**: Cloudflare Pages deployment
- **cloudflare_workers**: Cloudflare Workers deployment
- **container**: Container deployment

## Deployment Status

- `pending`: Waiting to deploy
- `deploying`: Currently deploying
- `success`: Deployed successfully
- `failed`: Deployment failed

## Permissions

- **Trigger Build**: User phải có `ViewProject` permission
- **List Artifacts**: User phải có `ViewProject` permission
- **Get Preview**: User phải có `ViewProject` permission
- **Trigger Deploy**: User phải có `ManageProject` permission
- **List Deployments**: User phải có `ViewProject` permission
- **Get Deployment**: User phải có `ViewProject` permission
