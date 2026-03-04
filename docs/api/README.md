# API Documentation

Tài liệu API chi tiết được tổ chức theo từng tính năng. Mỗi file chứa thông tin đầy đủ về request/response, error handling, validation rules, và cách sử dụng.

## Cấu trúc

Mỗi file trong thư mục này mô tả một nhóm API endpoints:

- **00-index.md**: Tổng quan và index
- **01-authentication.md**: Authentication APIs
- **02-health-check.md**: Health check endpoints
- **03-dashboard.md**: Dashboard APIs
- **04-users.md**: User management APIs
- **05-projects.md**: Project management APIs
- **06-tasks.md**: Task management APIs
- **07-task-attempts.md**: Task execution APIs
- **21-project-assistant.md**: Project Assistant APIs
- **22-agent-provider-auth.md**: Agent Provider Auth APIs
- **23-github.md**: GitHub Integration
- ... và các file khác

## Format của mỗi file

Mỗi file API documentation bao gồm:

1. **Base Path**: Base path cho nhóm API
2. **Authentication**: Yêu cầu authentication
3. **Endpoints**: Chi tiết từng endpoint:
   - Request format (headers, body, query params, path params)
   - Response format (success và error)
   - Validation rules
   - Error codes và messages
   - Frontend usage (file, component, màn hình)
   - Backend implementation (file, function)
   - Examples

## Sử dụng

### Cho Frontend Developers

1. Tìm endpoint bạn cần trong file tương ứng
2. Xem Request format để biết cách gọi API
3. Xem Response format để biết cách handle response
4. Xem Frontend Usage để biết cách integrate vào code
5. Xem Error Responses để handle errors đúng cách

### Cho Backend Developers

1. Tìm endpoint trong file tương ứng
2. Xem Backend Implementation để biết file và function cần modify
3. Xem Validation Rules để implement validation
4. Xem Error Codes để return đúng error codes

## Examples

### Frontend Example

```typescript
import { apiGet, apiPost } from '@/api/client';

// Get dashboard data
const dashboardData = await apiGet<DashboardData>('/dashboard');

// Create project
const project = await apiPost<Project>('/projects', {
  name: 'My Project',
  description: 'Project description'
});
```

### Backend Example

```rust
#[utoipa::path(
    get,
    path = "/api/v1/dashboard",
    tag = "Dashboard",
    responses(
        (status = 200, description = "Get dashboard data", body = DashboardResponse)
    )
)]
pub async fn get_dashboard(
    State(pool): State<PgPool>,
    auth_user: AuthUser,
) -> ApiResult<Json<ApiResponse<DashboardData>>> {
    // Implementation
}
```

## Tổng hợp nhanh

Xem file `../api-integration-documentation.md` để có overview tổng hợp của tất cả APIs.

## Contributing

Khi thêm API mới:

1. Tạo hoặc update file tương ứng trong thư mục này
2. Follow format của các file hiện có
3. Include đầy đủ:
   - Request/Response examples
   - Error cases
   - Frontend usage
   - Backend implementation
   - Validation rules
