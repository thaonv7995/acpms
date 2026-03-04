# Preview API

API endpoints cho preview environments.

## Base Path

`/api/v1/attempts/:id/preview` và `/api/v1/previews`

## Authentication

Preview routes đã enforce JWT + RBAC ở route level.

---

## Endpoints

### 1. POST `/api/v1/attempts/:id/preview`

Start preview cho attempt (Cloudflare tunnel + Docker runtime).

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

Không có

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "attempt_id": "550e8400-e29b-41d4-a716-446655440001",
  "preview_url": "https://task-550e8400.example.com",
  "status": "active",
  "created_at": "2026-01-13T10:00:00Z",
  "expires_at": "2026-01-20T10:00:00Z"
}
```

**Note**: Endpoint này trả về trực tiếp `PreviewInfo` (không bọc `ApiResponse`).

**Behavior quan trọng**:
- Idempotent: nếu preview đã tồn tại thì trả lại preview hiện có.
- Có advisory lock để tránh race khi start đồng thời.
- Bị chặn nếu:
  - Project type không hỗ trợ preview.
  - `project.settings.preview_enabled = false`.
  - Thiếu Cloudflare config bắt buộc.
  - Docker preview runtime đang disabled.

#### Frontend Usage

**File**: `frontend/src/api/previews.ts`

**Màn hình**: Project Tasks Preview Panel

**Backend**: `crates/server/src/routes/preview.rs::create_preview`

---

### 2. GET `/api/v1/attempts/:id/preview`

Lấy preview hiện có của attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "attempt_id": "550e8400-e29b-41d4-a716-446655440001",
  "preview_url": "https://task-550e8400.example.com",
  "status": "active",
  "created_at": "2026-01-13T10:00:00Z",
  "expires_at": "2026-01-20T10:00:00Z"
}
```

Hoặc `null` nếu chưa có preview.

**Backend**: `crates/server/src/routes/preview.rs::get_preview_for_attempt`

---

### 3. GET `/api/v1/attempts/:id/preview/readiness`

Kiểm tra attempt có đủ điều kiện mở preview không.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "attempt_id": "550e8400-e29b-41d4-a716-446655440001",
  "project_type": "web",
  "preview_supported": true,
  "preview_enabled": true,
  "runtime_enabled": true,
  "cloudflare_ready": true,
  "ready": true,
  "missing_cloudflare_fields": [],
  "reason": null
}
```

**Backend**: `crates/server/src/routes/preview.rs::get_preview_readiness_for_attempt`

---

### 4. GET `/api/v1/attempts/:id/preview/runtime-status`

Lấy trạng thái Docker runtime của preview (debug/ops).

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "attempt_id": "550e8400-e29b-41d4-a716-446655440001",
  "runtime_enabled": true,
  "worktree_path": "/path/to/worktree",
  "compose_file_exists": true,
  "docker_project_name": "acpms-preview-550e8400",
  "compose_file_path": "/path/to/worktree/.acpms/preview/550e8400-e29b-41d4-a716-446655440001/docker-compose.preview.yml",
  "running_services": ["dev-server", "cloudflared"],
  "runtime_ready": true,
  "last_error": null,
  "started_at": "2026-02-27T06:45:00Z",
  "stopped_at": null,
  "message": null
}
```

**Backend**: `crates/server/src/routes/preview.rs::get_preview_runtime_status_for_attempt`

---

### 5. GET `/api/v1/attempts/:id/preview/runtime-logs`

Lấy log runtime Docker compose của preview (debug/ops).

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Query Parameters

- `tail` (integer, optional, default `200`, min `1`, max `2000`): Số dòng log cuối cần lấy.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "attempt_id": "550e8400-e29b-41d4-a716-446655440001",
  "runtime_enabled": true,
  "docker_project_name": "acpms-preview-550e8400",
  "compose_file_path": "/path/to/worktree/.acpms/preview/550e8400-e29b-41d4-a716-446655440001/docker-compose.preview.yml",
  "tail": 200,
  "logs": "dev-server  | VITE v5.0.0 ready in 350 ms\ncloudflared | Registered tunnel connection",
  "message": null
}
```

**Backend**: `crates/server/src/routes/preview.rs::get_preview_runtime_logs_for_attempt`

---

### 6. GET `/api/v1/previews`

Lấy danh sách active previews.

#### Response

**Status**: `200 OK`

**Body**:
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "attempt_id": "550e8400-e29b-41d4-a716-446655440001",
    "preview_url": "https://task-550e8400.example.com",
    "status": "active",
    "created_at": "2026-01-13T10:00:00Z",
    "expires_at": "2026-01-20T10:00:00Z"
  }
]
```

**Note**: Endpoint này trả về trực tiếp `Vec<PreviewInfo>` (không bọc `ApiResponse`).

#### Frontend Usage

**File**: `frontend/src/api/previews.ts`

**Màn hình**: Settings Page

**Backend**: `crates/server/src/routes/preview.rs::list_previews`

---

### 7. DELETE `/api/v1/previews/:id`

Stop preview và cleanup environment.

#### Path Parameters

- `id` (UUID, required): Có thể là `attempt_id` hoặc `preview_id` (`cloudflare_tunnels.id`)

#### Response

**Status**: `200 OK`

**Body**: Empty

**Behavior**:
- Stop Docker runtime (nếu enabled).
- Xóa Cloudflare tunnel + DNS record (best effort).
- Soft-delete bản ghi preview trong DB.

#### Frontend Usage

**File**: `frontend/src/api/previews.ts`

**Màn hình**: Settings Page

**Backend**: `crates/server/src/routes/preview.rs::cleanup_preview`

---

## Preview Lifecycle

1. **Readiness Check**: FE gọi readiness để quyết định cho mở preview mode hay không.
2. **Start**: API start tạo/khôi phục preview, start Docker runtime, trả public URL.
3. **Runtime Check**: Có thể dùng runtime-status để debug container/service.
4. **Runtime Logs**: Có thể dùng runtime-logs để xem log service nhanh.
5. **Access**: URL preview dùng để nhúng iframe trong panel.
6. **Cleanup**: Manual stop hoặc cleanup job khi hết TTL.

## Permissions

- **Create Preview**: `ExecuteTask`
- **Get Preview/Readiness/Runtime Status/Runtime Logs**: `ViewProject`
- **List Previews**: `System Admin`
- **Cleanup Preview**: `ManageProject`
