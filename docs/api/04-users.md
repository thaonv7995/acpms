# Users API

API endpoints cho quản lý users.

## Base Path

`/api/v1/users`

## Authentication

Auth được áp dụng theo từng endpoint:
- `PUT /users/:id`, `PUT /users/:id/password`, `POST /users/avatar/upload-url`: yêu cầu JWT.
- `GET /users`, `GET /users/:id`, `DELETE /users/:id`: hiện chưa bắt buộc JWT ở handler.

---

## Endpoints

### 1. GET `/api/v1/users`

Lấy danh sách tất cả users.

#### Request

Không yêu cầu header auth ở implementation hiện tại.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Users retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "email": "user@example.com",
      "name": "User Name",
      "avatar_url": "https://...",
      "gitlab_username": "username",
      "global_roles": ["viewer"],
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

**Note**: Avatar URLs được convert từ S3 keys thành presigned URLs (expires in 1 hour).

#### Frontend Usage

**File**: `frontend/src/api/users.ts`

**Màn hình**: User Management Page (Admin only)

**Backend**: `crates/server/src/routes/users.rs::list_users`

---

### 2. GET `/api/v1/users/:id`

Lấy thông tin user theo ID.

#### Path Parameters

- `id` (UUID, required): User ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "User retrieved successfully",
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com",
    "name": "User Name",
    "avatar_url": "https://...",
    "gitlab_username": null,
    "global_roles": ["viewer"],
    "created_at": "2026-01-13T10:00:00Z"
  }
}
```

#### Error Responses

**404 Not Found**:
```json
{
  "success": false,
  "code": "4043",
  "message": "User not found"
}
```

#### Frontend Usage

**File**: `frontend/src/pages/ProfilePage.tsx`

**Màn hình**: Profile Page (`/profile`)

**Backend**: `crates/server/src/routes/users.rs::get_user`

---

### 3. PUT `/api/v1/users/:id`

Cập nhật thông tin user.

#### Path Parameters

- `id` (UUID, required): User ID

#### Request Body

```json
{
  "name": "New Name",
  "avatar_url": "https://...",
  "gitlab_username": "username",
  "global_roles": ["viewer", "admin"]
}
```

**Fields** (all optional):
- `name` (string): 1-100 characters
- `avatar_url` (string): Valid URL format
- `gitlab_username` (string): 1-50 characters
- `global_roles` (array): Array of SystemRole

**Validation**:
- Name: 1-100 characters
- Avatar URL: Valid URL format
- GitLab username: 1-50 characters

**Permissions**:
- User có thể update chính mình
- Admin có thể update bất kỳ user nào
- Chỉ admin có thể update `global_roles`

#### Response

**Status**: `200 OK`

**Body**: UserDto object

#### Error Responses

**403 Forbidden** - Không có quyền update roles:
```json
{
  "success": false,
  "code": "4030",
  "message": "Forbidden - Only admins can modify roles"
}
```

#### Frontend Usage

**File**: `frontend/src/pages/ProfilePage.tsx`

**Màn hình**: Profile Page

**Backend**: `crates/server/src/routes/users.rs::update_user`

---

### 4. DELETE `/api/v1/users/:id`

Xóa user.

#### Path Parameters

- `id` (UUID, required): User ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "User deleted successfully"
}
```

**Implementation Note**: Handler hiện chưa enforce admin role cho endpoint này.

#### Frontend Usage

**File**: `frontend/src/pages/UserManagementPage.tsx`

**Màn hình**: User Management Page (Admin only)

**Backend**: `crates/server/src/routes/users.rs::delete_user`

---

### 5. PUT `/api/v1/users/:id/password`

Đổi mật khẩu.

#### Path Parameters

- `id` (UUID, required): User ID

#### Request Body

```json
{
  "current_password": "oldpassword123",
  "new_password": "newpassword123"
}
```

**Fields**:
- `current_password` (string, required): Mật khẩu hiện tại
- `new_password` (string, required): Mật khẩu mới, tối thiểu 8 characters

**Validation**:
- Current password: Required
- New password: Minimum 8 characters, must contain at least one letter and one number

**Permissions**: User chỉ có thể đổi mật khẩu của chính mình.

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Password changed successfully"
}
```

#### Error Responses

**401 Unauthorized** - Current password không đúng:
```json
{
  "success": false,
  "code": "4011",
  "message": "Invalid current password"
}
```

**400 Bad Request** - Validation error:
```json
{
  "success": false,
  "code": "4001",
  "message": "Validation error",
  "error": {
    "details": "New password must be at least 8 characters"
  }
}
```

#### Frontend Usage

**File**: `frontend/src/pages/ProfilePage.tsx`

**Màn hình**: Profile Page

**Backend**: `crates/server/src/routes/users.rs::change_password`

---

### 6. POST `/api/v1/users/avatar/upload-url`

Lấy presigned URL để upload avatar.

#### Request Body

```json
{
  "filename": "avatar.jpg",
  "content_type": "image/jpeg"
}
```

**Fields**:
- `filename` (string, required): Tên file
- `content_type` (string, required): MIME type (ví dụ: "image/jpeg", "image/png")

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Upload URL generated successfully",
  "data": {
    "upload_url": "https://minio.example.com/bucket/avatars/...?signature=...",
    "key": "avatars/550e8400-e29b-41d4-a716-446655440000/avatar.jpg"
  }
}
```

**Fields**:
- `upload_url` (string): Presigned URL để upload (expires in 1 hour)
- `key` (string): S3 key để lưu trong database

#### Frontend Usage

**File**: `frontend/src/pages/ProfilePage.tsx`

**Example**:
```typescript
// 1. Get upload URL
const { upload_url, key } = await apiPost<UploadUrlResponse>('/users/avatar/upload-url', {
  filename: file.name,
  content_type: file.type
});

// 2. Upload file to presigned URL
await fetch(upload_url, {
  method: 'PUT',
  body: file,
  headers: {
    'Content-Type': file.type
  }
});

// 3. Update user with avatar key
await apiPut(`/users/${userId}`, {
  avatar_url: key
});
```

**Màn hình**: Profile Page

**Backend**: `crates/server/src/routes/users.rs::get_avatar_upload_url`

---

## Avatar URL Handling

Avatar URLs được xử lý đặc biệt:

1. **S3 Keys**: Nếu `avatar_url` là S3 key (không phải full URL), server sẽ convert thành presigned URL
2. **Full URLs**: Nếu `avatar_url` đã là full URL (http/https), server sẽ return as-is
3. **Presigned URLs**: Expires in 1 hour, được regenerate mỗi lần fetch

**Storage Service**: `acpms_services::StorageService`

---

## Permissions

- **List Users**: Tất cả authenticated users
- **Get User**: Tất cả authenticated users
- **Update User**: 
  - User có thể update chính mình
  - Admin có thể update bất kỳ user nào
  - Chỉ admin có thể update `global_roles`
- **Delete User**: Admin only
- **Change Password**: User chỉ có thể đổi mật khẩu của chính mình
- **Get Upload URL**: Tất cả authenticated users
