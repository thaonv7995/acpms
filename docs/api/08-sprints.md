# Sprints API

API endpoints cho sprint management.

## Base Path

`/api/v1/projects/:project_id/sprints`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. GET `/api/v1/projects/:project_id/sprints`

Lấy danh sách sprints của project.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Sprints retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "project_id": "...",
      "name": "Sprint 1",
      "start_date": "2026-01-01T00:00:00Z",
      "end_date": "2026-01-14T00:00:00Z",
      "status": "active"
    }
  ]
}
```

**Permissions**: User phải có `ViewProject` permission.

#### Frontend Usage

**File**: `frontend/src/hooks/useSprints.ts`

**Màn hình**: Project Detail Page - Sprints Tab

**Backend**: `crates/server/src/routes/sprints.rs::list_project_sprints`

---

### 2. POST `/api/v1/projects/:project_id/sprints`

Tạo sprint mới.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Request Body

```json
{
  "name": "Sprint 1",
  "start_date": "2026-01-01T00:00:00Z",
  "end_date": "2026-01-14T00:00:00Z"
}
```

**Fields**:
- `name` (string, required): Sprint name
- `start_date` (datetime, required): Start date
- `end_date` (datetime, required): End date

**Permissions**: User phải có `ManageSprints` permission.

#### Response

**Status**: `201 Created`

**Body**: SprintDto object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Sprints Tab

**Backend**: `crates/server/src/routes/sprints.rs::create_sprint`

---

### 3. POST `/api/v1/projects/:project_id/sprints/generate`

Auto-generate sprints từ tasks.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Request Body

```json
{
  "start_date": "2026-01-01T00:00:00Z",
  "sprint_duration_days": 14
}
```

**Fields**:
- `start_date` (datetime, required): Start date của sprint đầu tiên
- `sprint_duration_days` (number, required): Duration của mỗi sprint (days)

#### Response

**Status**: `201 Created`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Sprints generated successfully",
  "data": {
    "sprints": [...],
    "total_generated": 4
  }
}
```

**Permissions**: User phải có `ManageSprints` permission.

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Sprints Tab

**Backend**: `crates/server/src/routes/sprints.rs::generate_sprints`

---

### 4. GET `/api/v1/projects/:project_id/sprints/active`

Lấy active sprint.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**: SprintDto object hoặc null nếu không có active sprint

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page

**Backend**: `crates/server/src/routes/sprints.rs::get_active_sprint`

---

### 5. GET `/api/v1/projects/:project_id/sprints/:sprint_id`

Lấy thông tin sprint theo ID.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `sprint_id` (UUID, required): Sprint ID

#### Response

**Status**: `200 OK`

**Body**: SprintDto object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Sprints Tab

**Backend**: `crates/server/src/routes/sprints.rs::get_sprint`

---

### 6. PUT `/api/v1/projects/:project_id/sprints/:sprint_id`

Cập nhật sprint.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `sprint_id` (UUID, required): Sprint ID

#### Request Body

```json
{
  "name": "Updated Sprint Name",
  "start_date": "2026-01-01T00:00:00Z",
  "end_date": "2026-01-14T00:00:00Z"
}
```

**Fields** (all optional):
- `name` (string)
- `start_date` (datetime)
- `end_date` (datetime)

**Permissions**: User phải có `ManageSprints` permission.

#### Response

**Status**: `200 OK`

**Body**: SprintDto object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Sprints Tab

**Backend**: `crates/server/src/routes/sprints.rs::update_sprint`

---

### 7. DELETE `/api/v1/projects/:project_id/sprints/:sprint_id`

Xóa sprint.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `sprint_id` (UUID, required): Sprint ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Sprint deleted successfully"
}
```

**Permissions**: User phải có `ManageSprints` permission.

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Sprints Tab

**Backend**: `crates/server/src/routes/sprints.rs::delete_sprint`

---

## Sprint Status

Valid statuses:
- `planned`: Planned but not started
- `active`: Currently active
- `completed`: Completed

## Permissions

- **List Sprints**: `ViewProject` permission
- **Create Sprint**: `ManageSprints` permission
- **Generate Sprints**: `ManageSprints` permission
- **Get Sprint**: `ViewProject` permission
- **Update Sprint**: `ManageSprints` permission
- **Delete Sprint**: `ManageSprints` permission
