# Projects API

API endpoints cho quản lý projects.

## Base Path

`/api/v1/projects`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. GET `/api/v1/projects`

Lấy danh sách projects của user hiện tại.

#### Request

**Headers**:
```
Authorization: Bearer <access_token>
```

**Query Parameters**: Không có

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Projects retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Project Name",
      "description": "Project description",
      "repository_url": "https://gitlab.com/...",
      "metadata": {},
      "architecture_config": {},
      "require_review": true,
      "settings": {
        "require_review": true,
        "timeout_mins": 60,
        "max_retries": 3,
        "auto_retry": false
      },
      "project_type": "web",
      "created_by": "550e8400-e29b-41d4-a716-446655440000",
      "created_at": "2026-01-13T10:00:00Z",
      "updated_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

**Note**: Chỉ trả về projects mà user là member.

#### Frontend Usage

**File**: `frontend/src/api/projects.ts`, `frontend/src/pages/ProjectsPage.tsx`

**Màn hình**: Projects Page (`/projects`)

**Backend**: `crates/server/src/routes/projects.rs::list_projects`

---

### 2. POST `/api/v1/projects`

Tạo project mới.

#### Request Body

```json
{
  "name": "Project Name",
  "description": "Project description",
  "repository_url": "https://gitlab.com/...",
  "metadata": {},
  "create_from_scratch": false,
  "visibility": "private",
  "require_review": true,
  "project_type": "web",
  "template_id": null
}
```

**Fields**:
- `name` (string, required): 1-100 characters
- `description` (string, optional): Max 500 characters
- `repository_url` (string, optional): Valid GitLab URL
- `metadata` (object, optional): JSON metadata
- `create_from_scratch` (boolean, optional): Tạo project từ đầu với GitLab repo mới
- `visibility` (string, optional): "private" | "public" | "internal" (cho from-scratch)
- `require_review` (boolean, optional): Require review trước khi commit
- `project_type` (string, optional): "web" | "mobile" | "desktop" | "extension" | "api" | "microservice"
- `template_id` (UUID, optional): Template ID để clone

**Validation**:
- Name: 1-100 characters
- Description: Max 500 characters
- Repository URL: Valid URL format
- Visibility: Must be "private", "public", or "internal"

**Permissions**: User phải có quyền tạo project.

#### Response

**Status**: `201 Created`

**Body**: ProjectDto object

**Note**: Nếu `create_from_scratch = true`, sẽ tự động tạo init task và bắt đầu execution.

#### Frontend Usage

**File**: `frontend/src/pages/ProjectsPage.tsx`

**Màn hình**: Projects Page - Create Project Modal

**Backend**: `crates/server/src/routes/projects.rs::create_project`

---

### 3. POST `/api/v1/projects/import`

Import project từ GitLab repository.

#### Request Body

```json
{
  "name": "Project Name",
  "repository_url": "https://gitlab.com/user/repo",
  "description": "Project description"
}
```

**Fields**:
- `name` (string, required): Project name
- `repository_url` (string, required): GitLab repository URL
- `description` (string, optional): Project description

**Validation**:
- Repository URL phải là GitLab URL hợp lệ
- URL phải từ allowed GitLab instances

#### Response

**Status**: `201 Created`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Project import started successfully",
  "data": {
    "project": { ... },
    "init_task_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**Note**: Tự động tạo init task để import code từ GitLab.

#### Frontend Usage

**File**: `frontend/src/pages/ProjectsPage.tsx`

**Màn hình**: Projects Page - Import Project Modal

**Backend**: `crates/server/src/routes/projects.rs::import_project`

---

### 4. GET `/api/v1/projects/:id`

Lấy thông tin project theo ID.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**: ProjectDto object

#### Error Responses

**404 Not Found**:
```json
{
  "success": false,
  "code": "4042",
  "message": "Project not found"
}
```

**403 Forbidden**:
```json
{
  "success": false,
  "code": "4030",
  "message": "Forbidden"
}
```

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page (`/projects/:id`)

**Backend**: `crates/server/src/routes/projects.rs::get_project`

---

### 5. PUT `/api/v1/projects/:id`

Cập nhật project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Request Body

```json
{
  "name": "New Name",
  "description": "New description",
  "repository_url": "https://...",
  "metadata": {},
  "require_review": false
}
```

**Fields** (all optional):
- `name` (string): 1-100 characters
- `description` (string): Max 500 characters
- `repository_url` (string): Valid URL
- `metadata` (object): JSON metadata
- `require_review` (boolean): Require review setting

**Permissions**: User phải có `ManageProject` permission (Owner hoặc Admin).

#### Response

**Status**: `200 OK`

**Body**: ProjectDto object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page

**Backend**: `crates/server/src/routes/projects.rs::update_project`

---

### 6. DELETE `/api/v1/projects/:id`

Xóa project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Project deleted successfully"
}
```

**Permissions**: User phải có `ManageProject` permission (Owner hoặc Admin).

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page

**Backend**: `crates/server/src/routes/projects.rs::delete_project`

---

### 7. GET `/api/v1/projects/:id/architecture`

Lấy architecture configuration của project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Architecture config retrieved successfully",
  "data": {
    "config": {
      "components": [...],
      "dependencies": [...]
    }
  }
}
```

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Architecture Tab

**Backend**: `crates/server/src/routes/projects.rs::get_architecture`

---

### 8. PUT `/api/v1/projects/:id/architecture`

Cập nhật architecture configuration.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Request Body

```json
{
  "config": {
    "components": [...],
    "dependencies": [...]
  }
}
```

**Fields**:
- `config` (object, required): Architecture configuration JSON

**Permissions**: User phải có `ManageProject` permission.

#### Response

**Status**: `200 OK`

**Body**: Architecture config object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Architecture Tab

**Backend**: `crates/server/src/routes/projects.rs::update_architecture`

---

### 9. GET `/api/v1/projects/:id/settings`

Lấy project settings.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Project settings retrieved successfully",
  "data": {
    "require_review": true,
    "timeout_mins": 60,
    "max_retries": 3,
    "auto_retry": false
  }
}
```

**Fields**:
- `require_review` (boolean): Require human review trước khi commit
- `timeout_mins` (number): Timeout cho task execution (minutes)
- `max_retries` (number): Maximum retry attempts
- `auto_retry` (boolean): Auto retry on failure

#### Frontend Usage

**File**: `frontend/src/api/projectSettings.ts`

**Màn hình**: Project Detail Page - Settings Tab

**Backend**: `crates/server/src/routes/projects.rs::get_project_settings`

---

### 10. PUT `/api/v1/projects/:id/settings`

Cập nhật project settings.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Request Body

```json
{
  "require_review": true,
  "timeout_mins": 60,
  "max_retries": 3,
  "auto_retry": false
}
```

**Fields** (all optional):
- `require_review` (boolean)
- `timeout_mins` (number)
- `max_retries` (number)
- `auto_retry` (boolean)

**Permissions**: User phải có `ManageProject` permission.

#### Response

**Status**: `200 OK`

**Body**: ProjectSettings object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Settings Tab

**Backend**: `crates/server/src/routes/projects.rs::update_project_settings`

---

### 11. PATCH `/api/v1/projects/:id/settings/:key`

Cập nhật một setting cụ thể.

#### Path Parameters

- `id` (UUID, required): Project ID
- `key` (string, required): Setting key ("require_review", "timeout_mins", "max_retries", "auto_retry")

#### Request Body

```json
{
  "value": true
}
```

**Fields**:
- `value` (any, required): Setting value

#### Response

**Status**: `200 OK`

**Body**: Updated settings object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Settings Tab

**Backend**: `crates/server/src/routes/projects.rs::update_single_project_setting`

---

### 12. POST `/api/v1/projects/import/preflight`

Pre-check trước khi import project.

#### Request Body

```json
{
  "repository_url": "https://github.com/user/repo"
}
```

#### Response

**Status**: `200 OK`

**Body**: Preflight check result (repository accessibility, permissions, conflicts).

**Backend**: `crates/server/src/routes/projects.rs::import_project_preflight`

---

### 13. POST `/api/v1/projects/import/create-fork`

Tạo fork khi import project (nếu user không có write access).

#### Request Body

```json
{
  "repository_url": "https://github.com/user/repo",
  "name": "Project Name"
}
```

#### Response

**Status**: `201 Created`

**Backend**: `crates/server/src/routes/projects.rs::import_project_create_fork`

---

### 14. POST `/api/v1/projects/init-refs/upload-url`

Lấy presigned URL để upload init reference files.

#### Request Body

```json
{
  "filename": "init-ref.tar.gz",
  "content_type": "application/gzip"
}
```

#### Response

**Status**: `200 OK`

**Body**: Upload URL và key.

**Backend**: `crates/server/src/routes/projects.rs::get_init_ref_upload_url`

---

### 15. POST `/api/v1/projects/:id/repository-context/recheck`

Recheck repository access cho project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "data": {
    "access_mode": "owner",
    "verification_status": "verified"
  }
}
```

**Backend**: `crates/server/src/routes/projects.rs::recheck_project_repository_access`

---

### 16. POST `/api/v1/projects/:id/repository-context/link-fork`

Link existing fork cho project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Request Body

```json
{
  "fork_url": "https://github.com/user/forked-repo"
}
```

**Backend**: `crates/server/src/routes/projects.rs::link_existing_fork`

---

### 17. POST `/api/v1/projects/:id/repository-context/create-fork`

Tạo fork mới cho project.

#### Path Parameters

- `id` (UUID, required): Project ID

**Backend**: `crates/server/src/routes/projects.rs::create_project_fork`

---

### 18. POST `/api/v1/projects/:id/sync`

Sync project repository (pull latest changes).

#### Path Parameters

- `id` (UUID, required): Project ID

**Backend**: `crates/server/src/routes/projects.rs::sync_project_repository`

---

### 19. GET `/api/v1/projects/:id/inviteable-users`

Lấy danh sách users có thể invite vào project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "data": [
    {
      "id": "...",
      "name": "John Doe",
      "email": "john@example.com",
      "avatar_url": "..."
    }
  ]
}
```

**Permissions**: `ManageMembers` (Owner).

**Backend**: `crates/server/src/routes/projects.rs::list_inviteable_users`

---

### 20. GET `/api/v1/projects/:id/members`

Lấy danh sách members của project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "data": [
    {
      "user_id": "...",
      "username": "john",
      "email": "john@example.com",
      "avatar_url": "...",
      "role": "owner",
      "joined_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

**Roles**: `owner`, `admin`, `developer`, `viewer`

**Backend**: `crates/server/src/routes/projects.rs::list_project_members`

---

### 21. POST `/api/v1/projects/:id/members`

Thêm member vào project.

#### Path Parameters

- `id` (UUID, required): Project ID

#### Request Body

```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "role": "developer"
}
```

**Permissions**: `ManageMembers` (Owner only).

#### Response

**Status**: `201 Created`

**Body**: ProjectMemberDto object.

**Backend**: `crates/server/src/routes/projects.rs::add_project_member`

---

### 22. PUT `/api/v1/projects/:id/members/:user_id`

Cập nhật role của member.

#### Path Parameters

- `id` (UUID, required): Project ID
- `user_id` (UUID, required): Target user ID

#### Request Body

```json
{
  "role": "admin"
}
```

**Permissions**: `ManageMembers` (Owner only). Không thể thay đổi role của owner.

**Backend**: `crates/server/src/routes/projects.rs::update_project_member`

---

### 23. DELETE `/api/v1/projects/:id/members/:user_id`

Xóa member khỏi project.

#### Path Parameters

- `id` (UUID, required): Project ID
- `user_id` (UUID, required): Target user ID

**Permissions**: `ManageMembers` (Owner only). Không thể xóa owner.

**Backend**: `crates/server/src/routes/projects.rs::remove_project_member`

---

## Permissions

- **List Projects**: Tất cả authenticated users (chỉ thấy projects của mình)
- **Create Project**: Tất cả authenticated users
- **Get Project**: User phải là member (`ViewProject` permission)
- **Update Project**: User phải có `ManageProject` permission (Owner hoặc Admin)
- **Delete Project**: User phải có `ManageProject` permission (Owner hoặc Admin)
- **Architecture**: `ViewProject` để get, `ManageProject` để update
- **Settings**: `ViewProject` để get, `ManageProject` để update
- **Members**: `ViewProject` để list, `ManageMembers` để add/update/remove (Owner only)
- **Repository Context**: `ManageProject` cho recheck/link-fork/create-fork/sync

---

## Project Types

Valid project types:
- `web`: Web application
- `mobile`: Mobile application
- `desktop`: Desktop application
- `extension`: Browser extension
- `api`: API service
- `microservice`: Microservice

---

## From-Scratch Projects

Khi `create_from_scratch = true`:
1. Tạo GitLab/GitHub repository mới với visibility đã chọn
2. Tạo init task với type `Init`
3. Tự động bắt đầu execution
4. Agent sẽ initialize project structure từ template

---

## Import Projects

Khi import từ GitLab/GitHub:
1. Pre-check via `/import/preflight`
2. Tạo fork nếu cần via `/import/create-fork`
3. Tạo project record via `/import`
4. Tạo init task với type `Init`
5. Tự động bắt đầu execution
6. Agent sẽ clone và analyze repository
