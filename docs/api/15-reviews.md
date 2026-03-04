# Reviews API

API endpoints cho code review và comments.

## Base Path

`/api/v1/attempts/:id/comments` và `/api/v1/comments/:id`

## Authentication

Tất cả endpoints yêu cầu JWT Bearer Token.

---

## Endpoints

### 1. GET `/api/v1/attempts/:id/comments`

Lấy danh sách review comments.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Review comments retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "attempt_id": "...",
      "user_id": "...",
      "content": "This needs improvement",
      "file_path": "src/file.ts",
      "line_number": 10,
      "resolved": false,
      "created_at": "2026-01-13T10:00:00Z"
    }
  ]
}
```

**Permissions**: User phải có `ViewProject` permission.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page - Review Tab

**Backend**: `crates/server/src/routes/reviews.rs::list_comments`

---

### 2. POST `/api/v1/attempts/:id/comments`

Thêm review comment.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "content": "This needs improvement",
  "file_path": "src/file.ts",
  "line_number": 10
}
```

**Fields**:
- `content` (string, required): Comment content
- `file_path` (string, optional): File path
- `line_number` (number, optional): Line number

**Permissions**: User phải có `ViewProject` permission.

#### Response

**Status**: `201 Created`

**Body**: ReviewCommentDto object

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page - Review Tab

**Backend**: `crates/server/src/routes/reviews.rs::add_comment`

---

### 3. DELETE `/api/v1/comments/:id`

Xóa comment.

#### Path Parameters

- `id` (UUID, required): Comment ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Comment deleted successfully"
}
```

**Permissions**: User chỉ có thể xóa comment của chính mình.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page - Review Tab

**Backend**: `crates/server/src/routes/reviews.rs::delete_comment`

---

### 4. PATCH `/api/v1/comments/:id/resolve`

Resolve comment.

#### Path Parameters

- `id` (UUID, required): Comment ID

#### Response

**Status**: `200 OK`

**Body**: ReviewCommentDto object

**Permissions**: User phải có `ViewProject` permission.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page - Review Tab

**Backend**: `crates/server/src/routes/reviews.rs::resolve_comment`

---

### 5. PATCH `/api/v1/comments/:id/unresolve`

Unresolve comment.

#### Path Parameters

- `id` (UUID, required): Comment ID

#### Response

**Status**: `200 OK`

**Body**: ReviewCommentDto object

**Permissions**: User phải có `ViewProject` permission.

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page - Review Tab

**Backend**: `crates/server/src/routes/reviews.rs::unresolve_comment`

---

### 6. POST `/api/v1/attempts/:id/request-changes`

Request changes cho attempt.

#### Path Parameters

- `id` (UUID, required): Attempt ID

#### Request Body

```json
{
  "feedback": "Please add error handling",
  "include_comments": true
}
```

**Fields**:
- `feedback` (string, required): Feedback message
- `include_comments` (boolean, optional): Include unresolved comments in feedback

**Permissions**: User phải có `ExecuteTask` permission.

#### Response

**Status**: `201 Created`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Changes requested, new attempt started with feedback",
  "data": {
    "original_attempt_id": "...",
    "new_attempt_id": "...",
    "feedback": "...",
    "comments_included": true
  }
}
```

**Note**: 
- Tạo attempt mới với feedback
- Update task status thành `in_progress`
- Submit job vào worker pool với high priority

#### Frontend Usage

**File**: `frontend/src/pages/TaskDetailPage.tsx`

**Màn hình**: Task Detail Page

**Backend**: `crates/server/src/routes/reviews.rs::request_changes`

---

## Comment States

- **Open**: Comment chưa được resolve
- **Resolved**: Comment đã được resolve

## Permissions

- **List Comments**: `ViewProject` permission
- **Add Comment**: `ViewProject` permission
- **Delete Comment**: User chỉ có thể xóa comment của chính mình
- **Resolve/Unresolve Comment**: `ViewProject` permission
- **Request Changes**: `ExecuteTask` permission
