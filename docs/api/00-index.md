# API Documentation Index

Tài liệu API chi tiết được tổ chức theo từng tính năng. Mỗi file chứa thông tin đầy đủ về request/response, error handling, và cách sử dụng.

## Cấu trúc tài liệu

### ✅ Đã hoàn thành

1. [Authentication](./01-authentication.md) - Đăng ký, đăng nhập, refresh token
2. [Health Check](./02-health-check.md) - Health check endpoints
3. [Dashboard](./03-dashboard.md) - Dashboard data và statistics
4. [Users](./04-users.md) - Quản lý users
5. [Projects](./05-projects.md) - Quản lý projects
6. [Tasks](./06-tasks.md) - Quản lý tasks
7. [Task Attempts](./07-task-attempts.md) - Task execution và attempts
8. [Sprints](./08-sprints.md) - Sprint management
9. [Requirements](./09-requirements.md) - Requirements management
10. [GitLab Integration](./10-gitlab.md) - GitLab integration và OAuth
11. [Settings](./11-settings.md) - System settings
12. [Preview](./12-preview.md) - Preview environments
13. [Deployments](./13-deployments.md) - Build và deployment
14. [Templates](./14-templates.md) - Project templates
15. [Reviews](./15-reviews.md) - Code review và comments
16. [Agent Activity](./16-agent-activity.md) - Agent status và logs
17. [WebSocket](./17-websocket.md) - WebSocket connections
18. [Admin](./18-admin.md) - Admin APIs
19. [Approvals](./19-approvals.md) - Tool approval workflow APIs
20. [Execution Processes](./20-execution-processes.md) - Process-centric execution/follow-up/reset APIs
21. [Project Assistant](./21-project-assistant.md) - AI assistant chat sessions
22. [Agent Provider Auth](./22-agent-provider-auth.md) - Agent CLI authentication
23. [GitHub Integration](./23-github.md) - GitHub integration (internal library)

## Authentication Methods

### JWT Bearer Token (REST API)

Nhiều REST endpoints sử dụng JWT Bearer Token trong Authorization header:

```
Authorization: Bearer <access_token>
```

**Token Storage (Frontend)**:
- Access Token: `localStorage.getItem('acpms_token')`
- Refresh Token: `localStorage.getItem('acpms_refresh_token')`

**Token Expiration**:
- Access Token: 30 minutes (default)
- Refresh Token: 7 days

### WebSocket Authentication

WebSocket connections yêu cầu JWT token qua:
- `Sec-WebSocket-Protocol`: `acpms-bearer, <jwt_token>` (browser clients)
- Hoặc Authorization header: `Authorization: Bearer <token>` (non-browser clients)

### GitLab Webhook Authentication

GitLab webhooks sử dụng custom header:
```
X-Gitlab-Token: <webhook_secret>
```

## Standard Response Format

Tất cả API responses đều wrap trong `ApiResponse<T>` format:

### Success Response

```json
{
  "success": true,
  "code": "0000",
  "message": "Operation completed successfully",
  "data": {
    // Response data here
  },
  "metadata": null,
  "error": null
}
```

### Error Response

```json
{
  "success": false,
  "code": "4001",
  "message": "Validation error",
  "data": null,
  "metadata": null,
  "error": {
    "details": "Field 'email' is required",
    "trace_id": "abc123-def456-ghi789"
  }
}
```

## Response Codes

### Success Codes
- `0000`: Success

### Client Errors (4xxx)
- `4000`: Bad Request
- `4001`: Validation Error
- `4002`: Missing Parameter
- `4003`: Invalid Format
- `4010`: Unauthorized
- `4011`: Invalid Credentials
- `4012`: Token Expired
- `4013`: Token Invalid
- `4030`: Forbidden
- `4031`: Access Denied
- `4040`: Not Found
- `4041`: Task Not Found
- `4042`: Project Not Found
- `4043`: User Not Found
- `4090`: Conflict
- `4091`: Resource Already Exists

### Server Errors (5xxx)
- `5000`: Internal Error
- `5001`: Database Error
- `5002`: External Service Error
- `5003`: Service Unavailable
- `5004`: Not Implemented

## Frontend API Client

**File**: `frontend/src/api/client.ts`

**Base URL**: `http://localhost:3000` (development)  
**API Prefix**: `/api/v1`

**Functions**:
- `apiGet<T>(path: string): Promise<T>`
- `apiPost<T>(path: string, data: unknown): Promise<T>`
- `apiPut<T>(path: string, data: unknown): Promise<T>`
- `apiPatch<T>(path: string, data: unknown): Promise<T>`
- `apiDelete(path: string): Promise<void>`
- `authenticatedFetch(path: string, options?: RequestInit): Promise<Response>`

**Auto Token Injection**: Tất cả requests tự động thêm `Authorization: Bearer <token>` header

**Error Handling**: 
- 401 Unauthorized → Clear tokens và redirect đến `/login`
- Standardized error format parsing

## Base Paths

- **REST API**: `/api/v1`
- **WebSocket**: `/ws`
- **Health Check**: `/health` (no prefix)

## Current Implementation Notes

- Không phải tất cả endpoints đều đã bật kiểm tra auth/role ở handler.
- `Auth` details mới nhất xem tại `01-authentication.md`.
- `Approvals` endpoints xem tại `19-approvals.md`.
- `Project Assistant` endpoints xem tại `21-project-assistant.md`.
- `Agent Provider Auth` endpoints xem tại `22-agent-provider-auth.md`.

## Rate Limiting

- Middleware rate-limit đã có trong codebase nhưng hiện chưa được gắn vào router mặc định.

## CORS

Server đang dùng `CorsLayer::permissive()` ở runtime.
