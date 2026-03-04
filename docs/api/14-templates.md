# Templates API

API endpoints cho project templates.

## Base Path

`/api/v1/templates`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. GET `/api/v1/templates`

Lấy danh sách project templates.

#### Query Parameters

- `project_type` (string, optional): Filter theo project type
- `official_only` (boolean, optional): Chỉ lấy official templates

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Templates retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Template Name",
      "description": "Template description",
      "project_type": "web",
      "official": true
    }
  ]
}
```

#### Frontend Usage

**File**: `frontend/src/api/templates.ts`

**Màn hình**: Projects Page - Create Project Modal

**Backend**: `crates/server/src/routes/templates.rs::list_templates`

---

### 2. GET `/api/v1/templates/:id`

Lấy thông tin template theo ID.

#### Path Parameters

- `id` (UUID, required): Template ID

#### Response

**Status**: `200 OK`

**Body**: TemplateDto object

#### Frontend Usage

**File**: `frontend/src/api/templates.ts`

**Màn hình**: Projects Page - Create Project Modal

**Backend**: `crates/server/src/routes/templates.rs::get_template`

---

### 3. POST `/api/v1/templates`

Tạo template mới (Admin only).

#### Request Body

```json
{
  "name": "Template Name",
  "description": "Template description",
  "project_type": "web",
  "template_data": {}
}
```

**Fields**:
- `name` (string, required): Template name
- `description` (string, optional): Template description
- `project_type` (string, required): Project type
- `template_data` (object, required): Template data

**Permissions**: Admin only.

#### Response

**Status**: `201 Created`

**Body**: TemplateDto object

#### Frontend Usage

Không có UI hiện tại (Admin only)

**Backend**: `crates/server/src/routes/templates.rs::create_template`

---

### 4. PUT `/api/v1/templates/:id`

Cập nhật template (Admin only).

#### Path Parameters

- `id` (UUID, required): Template ID

#### Request Body

```json
{
  "name": "Updated Name",
  "description": "Updated description"
}
```

**Fields** (all optional):
- `name` (string)
- `description` (string)
- `template_data` (object)

**Permissions**: Admin only.

#### Response

**Status**: `200 OK`

**Body**: TemplateDto object

#### Frontend Usage

Không có UI hiện tại (Admin only)

**Backend**: `crates/server/src/routes/templates.rs::update_template`

---

### 5. DELETE `/api/v1/templates/:id`

Xóa template (Admin only).

#### Path Parameters

- `id` (UUID, required): Template ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Template deleted successfully"
}
```

**Permissions**: Admin only.

#### Frontend Usage

Không có UI hiện tại (Admin only)

**Backend**: `crates/server/src/routes/templates.rs::delete_template`

---

## Template Usage

Templates được sử dụng khi tạo project với `template_id`:
1. Clone template structure
2. Initialize project với template files
3. Apply template configuration

## Permissions

- **List Templates**: Tất cả authenticated users
- **Get Template**: Tất cả authenticated users
- **Create Template**: Admin only
- **Update Template**: Admin only
- **Delete Template**: Admin only
