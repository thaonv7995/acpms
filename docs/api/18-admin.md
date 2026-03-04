# Admin API

API endpoints cho admin operations (Admin only).

## Base Path

`/api/v1/admin`

## Authentication

Thiết kế yêu cầu JWT + Admin role, nhưng handler hiện tại còn TODO phần enforce role.

---

## Endpoints

### 1. GET `/api/v1/admin/webhooks/failed`

Lấy danh sách failed webhooks (dead letter queue).

#### Query Parameters

- `project_id` (UUID, optional): Filter theo project ID
- `limit` (number, optional): Max results (default: 50, max: 200)

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Failed webhooks retrieved successfully",
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "project_id": "...",
      "event_id": "...",
      "event_type": "merge_request",
      "attempt_count": 3,
      "last_error": "Connection timeout",
      "created_at": "2026-01-13T10:00:00Z",
      "last_attempt_at": "2026-01-13T10:05:00Z"
    }
  ]
}
```

**Permissions**: Admin only.

#### Frontend Usage

Không có UI hiện tại (có thể thêm admin panel sau)

**Backend**: `crates/server/src/routes/webhooks-admin.rs::get_failed_webhooks`

---

### 2. POST `/api/v1/admin/webhooks/:id/retry`

Retry failed webhook.

#### Path Parameters

- `id` (UUID, required): Webhook event ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Webhook queued for retry"
}
```

**Note**: 
- Reset status về `pending`
- Reset attempt count về 0
- Allow reprocessing

**Permissions**: Admin only.

#### Frontend Usage

Không có UI hiện tại

**Backend**: `crates/server/src/routes/webhooks-admin.rs::retry_webhook`

---

### 3. GET `/api/v1/admin/webhooks/stats`

Lấy webhook statistics.

#### Query Parameters

- `project_id` (UUID, optional): Filter theo project ID

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Webhook statistics retrieved successfully",
  "data": {
    "pending": 5,
    "processing": 2,
    "completed": 100,
    "failed": 3
  }
}
```

**Fields**:
- `pending`: Số webhooks đang pending
- `processing`: Số webhooks đang processing
- `completed`: Số webhooks đã completed
- `failed`: Số webhooks đã failed

**Permissions**: Admin only.

#### Frontend Usage

Không có UI hiện tại

**Backend**: `crates/server/src/routes/webhooks-admin.rs::get_webhook_stats`

---

## Webhook Event States

- `pending`: Waiting to process
- `processing`: Currently processing
- `completed`: Processed successfully
- `failed`: Failed after max retries

## Permissions

Target behavior:
- Valid JWT Bearer Token
- User có `admin` role trong `global_roles`

Current implementation:
- Chưa enforce đầy đủ ở route handlers (cần bổ sung middleware/check role).

## Admin Operations

Admin có thể:
- View failed webhooks
- Retry failed webhooks
- View webhook statistics
- Monitor system health

## Notes

- Admin endpoints không có UI hiện tại
- Có thể được expose qua admin panel trong tương lai
- Useful cho debugging và monitoring
