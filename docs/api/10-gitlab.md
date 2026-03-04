# GitLab Integration API

API endpoints cho GitLab integration và OAuth.

## Base Path

`/api/v1/projects/:id/gitlab` và `/api/v1/gitlab/oauth`

## Authentication

`/webhooks/gitlab` dùng `X-Gitlab-Token`.  
Các endpoint còn lại hiện chưa enforce JWT ở handler (cần hardening thêm).

---

## Endpoints

### 1. POST `/api/v1/projects/:id/gitlab/link`

Link project với GitLab repository.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Request Body

```json
{
  "gitlab_project_id": 12345,
  "base_url": "https://gitlab.com"
}
```

**Fields**:
- `gitlab_project_id` (number, required): GitLab project ID
- `base_url` (string, required): GitLab instance base URL

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "GitLab project linked successfully",
  "data": {
    "project_id": "...",
    "gitlab_project_id": 12345,
    "base_url": "https://gitlab.com",
    "webhook_secret": "..."
  }
}
```

**Note**: Tự động tạo webhook secret để verify webhook requests.

#### Frontend Usage

**File**: `frontend/src/api/gitlab.ts`

**Màn hình**: Project Detail Page - Settings Tab

**Backend**: `crates/server/src/routes/gitlab.rs::link_project`

---

### 2. GET `/api/v1/projects/:id/gitlab/status`

Lấy GitLab configuration status.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "GitLab configuration retrieved successfully",
  "data": {
    "project_id": "...",
    "gitlab_project_id": 12345,
    "base_url": "https://gitlab.com",
    "webhook_secret": "..."
  }
}
```

**Note**: Returns `null` nếu project chưa được link với GitLab.

#### Frontend Usage

**File**: `frontend/src/api/gitlab.ts`

**Màn hình**: Project Detail Page - Settings Tab

**Backend**: `crates/server/src/routes/gitlab.rs::get_status`

---

### 3. GET `/api/v1/tasks/:id/gitlab/merge_requests`

Lấy merge requests của task.

#### Path Parameters

- `id` (UUID, required): Task ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Task merge requests retrieved successfully",
  "data": [
    {
      "id": "...",
      "iid": 123,
      "title": "Merge Request Title",
      "state": "opened",
      "web_url": "https://gitlab.com/...",
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/api/mergeRequests.ts`

**Màn hình**: 
- Task Detail Page
- Merge Request Page

**Backend**: `crates/server/src/routes/gitlab.rs::get_task_merge_requests`

---

### 4. POST `/api/v1/webhooks/gitlab`

GitLab webhook endpoint (được GitLab gọi).

#### Request

**Headers**:
```
X-Gitlab-Token: <webhook_secret>
```

**Body**: GitLab webhook payload (JSON)

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Webhook received and queued for processing"
}
```

**Note**: 
- Webhook được queue để xử lý async
- Response ngay lập tức (non-blocking)
- Processing xảy ra trong background

#### Frontend Usage

Không sử dụng trực tiếp (GitLab gọi endpoint này)

**Backend**: `crates/server/src/routes/gitlab.rs::handle_webhook`

---

## GitLab OAuth

### 5. GET `/api/v1/gitlab/oauth/authorize`

Bắt đầu GitLab OAuth flow.

#### Query Parameters

- `project_id` (UUID, optional): Project ID để link OAuth token

#### Response

**Status**: `302 Found` (Redirect)

**Location**: GitLab OAuth authorization URL

**Note**: 
- Redirect user đến GitLab để authorize
- State token được encode với project_id nếu có

#### Frontend Usage

**File**: `frontend/src/pages/SettingsPage.tsx`

**Màn hình**: Settings Page

**Backend**: `crates/server/src/routes/gitlab-oauth.rs::authorize`

---

### 6. GET `/api/v1/gitlab/oauth/callback`

GitLab OAuth callback.

#### Query Parameters

- `code` (string, required): Authorization code từ GitLab
- `state` (string, required): State token (CSRF protection)

#### Response

**Status**: `200 OK` hoặc `302 Found` (Redirect về frontend)

**Body**:
```json
{
  "success": true,
  "gitlab_user_id": 12345,
  "gitlab_username": "username",
  "expires_at": "2026-01-13T10:00:00Z"
}
```

**Note**: 
- Exchange authorization code cho access token
- Store encrypted token trong database
- Link với project nếu project_id trong state

#### Frontend Usage

Không sử dụng trực tiếp (OAuth callback)

**Backend**: `crates/server/src/routes/gitlab-oauth.rs::callback`

---

## Webhook Events

GitLab webhook hỗ trợ các event types:
- `merge_request`: Merge request events
- `push`: Push events
- `issue`: Issue events
- `note`: Comment events

## Webhook Security

- Webhook secret được generate tự động khi link project
- Secret được lưu trong database và GitLab webhook configuration
- Tất cả webhook requests phải có `X-Gitlab-Token` header matching secret

## Permissions

- **Link Project**: User phải có `ManageProject` permission
- **Get Status**: User phải có `ViewProject` permission
- **Get Merge Requests**: User phải có `ViewProject` permission
- **OAuth Authorize**: Tất cả authenticated users
- **OAuth Callback**: Public endpoint (GitLab redirect)
