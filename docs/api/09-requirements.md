# Requirements API

API endpoints cho requirements management.

## Base Path

`/api/v1/projects/:project_id/requirements`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. GET `/api/v1/projects/:project_id/requirements`

Lấy danh sách requirements của project.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Requirements retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "project_id": "...",
      "title": "Requirement Title",
      "description": "Requirement description",
      "priority": "high",
      "status": "open"
    }
  ]
}
```

**Permissions**: User phải có `ViewProject` permission.

#### Frontend Usage

**File**: `frontend/src/api/requirements.ts`

**Màn hình**: Project Detail Page - Requirements Tab

**Backend**: `crates/server/src/routes/requirements.rs::list_project_requirements`

---

### 2. POST `/api/v1/projects/:project_id/requirements`

Tạo requirement mới.

#### Path Parameters

- `project_id` (UUID, required): Project ID

#### Request Body

```json
{
  "title": "Requirement Title",
  "description": "Requirement description",
  "priority": "high"
}
```

**Fields**:
- `title` (string, required): Requirement title
- `description` (string, optional): Requirement description
- `priority` (string, optional): "low" | "medium" | "high"

**Permissions**: User phải có `CreateRequirement` permission.

#### Response

**Status**: `201 Created`

**Body**: RequirementDto object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Requirements Tab

**Backend**: `crates/server/src/routes/requirements.rs::create_requirement`

---

### 3. GET `/api/v1/projects/:project_id/requirements/:id`

Lấy thông tin requirement theo ID.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `id` (UUID, required): Requirement ID

#### Response

**Status**: `200 OK`

**Body**: RequirementDto object

**Permissions**: User phải có `ViewProject` permission.

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Requirements Tab

**Backend**: `crates/server/src/routes/requirements.rs::get_requirement`

---

### 4. PUT `/api/v1/projects/:project_id/requirements/:id`

Cập nhật requirement.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `id` (UUID, required): Requirement ID

#### Request Body

```json
{
  "title": "Updated Title",
  "description": "...",
  "priority": "medium"
}
```

**Fields** (all optional):
- `title` (string)
- `description` (string)
- `priority` (string)

**Permissions**: User phải có `ModifyRequirement` permission.

#### Response

**Status**: `200 OK`

**Body**: RequirementDto object

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Requirements Tab

**Backend**: `crates/server/src/routes/requirements.rs::update_requirement`

---

### 5. DELETE `/api/v1/projects/:project_id/requirements/:id`

Xóa requirement.

#### Path Parameters

- `project_id` (UUID, required): Project ID
- `id` (UUID, required): Requirement ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Requirement deleted successfully"
}
```

**Permissions**: User phải có `DeleteRequirement` permission.

#### Frontend Usage

**File**: `frontend/src/pages/ProjectDetailPage.tsx`

**Màn hình**: Project Detail Page - Requirements Tab

**Backend**: `crates/server/src/routes/requirements.rs::delete_requirement`

---

## Requirement Priority

Valid priorities:
- `low`: Low priority
- `medium`: Medium priority
- `high`: High priority

## Requirement Status

Valid statuses:
- `open`: Open requirement
- `in_progress`: In progress
- `completed`: Completed
- `closed`: Closed

## Permissions

- **List Requirements**: `ViewProject` permission
- **Create Requirement**: `CreateRequirement` permission
- **Get Requirement**: `ViewProject` permission
- **Update Requirement**: `ModifyRequirement` permission
- **Delete Requirement**: `DeleteRequirement` permission
