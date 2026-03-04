# API Integration Tests

Test suite cho tất cả API endpoints của ACPMS backend.

## Cấu trúc

```
tests/
├── mod.rs                  # Test module entry point
├── helpers.rs              # Test utilities và helpers
├── auth_tests.rs           # Authentication API tests
├── health_tests.rs         # Health check tests
├── users_tests.rs          # Users API tests
├── projects_tests.rs       # Projects API tests
├── tasks_tests.rs          # Tasks API tests
├── task_attempts_tests.rs  # Task attempts API tests
├── dashboard_tests.rs      # Dashboard API tests
├── sprints_tests.rs        # Sprints API tests
├── requirements_tests.rs   # Requirements API tests
├── reviews_tests.rs        # Reviews API tests
├── agent_activity_tests.rs # Agent Activity API tests
├── settings_tests.rs       # Settings API tests
├── preview_tests.rs        # Preview API tests
├── templates_tests.rs      # Templates API tests
├── admin_tests.rs          # Admin API tests
├── gitlab_tests.rs         # GitLab Integration API tests
└── deployments_tests.rs    # Deployments API tests
```

## Chạy Tests

### Setup Test Database

```bash
# Tạo test database
createdb acpms_test

# Set environment variable
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/acpms_test"
```

### Chạy tất cả tests

```bash
# Chạy tất cả tests (bao gồm ignored)
cargo test --package acpms-server -- --ignored

# Chạy tests không ignored
cargo test --package acpms-server

# Chạy với output
cargo test --package acpms-server -- --nocapture

# Chạy test cụ thể
cargo test --package acpms-server test_register_success
```

### Chạy test module cụ thể

```bash
cargo test --package acpms-server auth_tests
cargo test --package acpms-server projects_tests
```

## Test Coverage

**Total Test Cases**: ~80+ test cases covering all major API endpoints

### Authentication Tests (`auth_tests.rs`)
- ✅ Register user (success, duplicate email, validation errors)
- ✅ Login (success, invalid credentials)
- ✅ Refresh token (success, invalid token)
- ✅ Logout

### Health Check Tests (`health_tests.rs`)
- ✅ Health check endpoint
- ✅ Readiness check
- ✅ Liveness check

### Users Tests (`users_tests.rs`)
- ✅ List users
- ✅ Get user (success, not found)
- ✅ Update user
- ✅ Change password (success, invalid current password)
- ✅ Delete user (admin only)
- ✅ Get avatar upload URL

### Projects Tests (`projects_tests.rs`)
- ✅ Create project
- ✅ List projects
- ✅ Get project (success, not found)
- ✅ Update project
- ✅ Delete project
- ✅ Get project settings
- ✅ Update project settings
- ✅ Get architecture
- ✅ Update architecture

### Tasks Tests (`tasks_tests.rs`)
- ✅ Create task
- ✅ List tasks
- ✅ Get task
- ✅ Update task status
- ✅ Assign task
- ✅ Delete task

### Task Attempts Tests (`task_attempts_tests.rs`)
- ✅ Create task attempt
- ✅ Get task attempts
- ✅ Get attempt
- ✅ Get attempt logs
- ✅ Cancel attempt
- ✅ Get attempt diff

### Dashboard Tests (`dashboard_tests.rs`)
- ✅ Get dashboard (success, unauthorized)

### Sprints Tests (`sprints_tests.rs`)
- ✅ Create sprint
- ✅ List project sprints
- ✅ Get sprint
- ✅ Update sprint
- ✅ Delete sprint
- ✅ Get active sprint
- ✅ Generate sprints

### Requirements Tests (`requirements_tests.rs`)
- ✅ Create requirement
- ✅ List project requirements
- ✅ Get requirement
- ✅ Update requirement
- ✅ Delete requirement

### Reviews Tests (`reviews_tests.rs`)
- ✅ Add comment
- ✅ List comments
- ✅ Resolve comment
- ✅ Unresolve comment
- ✅ Delete comment
- ✅ Request changes

### Agent Activity Tests (`agent_activity_tests.rs`)
- ✅ Get agent status
- ✅ Get agent logs
- ✅ Get agent logs filtered by attempt
- ✅ Get agent logs filtered by project
- ✅ Get agent status unauthorized

### Settings Tests (`settings_tests.rs`)
- ✅ Get settings
- ✅ Update settings
- ✅ Get settings unauthorized

### Preview Tests (`preview_tests.rs`)
- ✅ List previews
- ✅ Create preview
- ✅ Cleanup preview

### Templates Tests (`templates_tests.rs`)
- ✅ List templates
- ✅ List templates filtered by type
- ✅ Get template
- ✅ Create template (admin)
- ✅ Get template not found

### Admin Tests (`admin_tests.rs`)
- ✅ Get failed webhooks
- ✅ Get failed webhooks filtered
- ✅ Get webhook stats
- ✅ Retry webhook
- ✅ Admin endpoints require auth

### GitLab Tests (`gitlab_tests.rs`)
- ✅ Get GitLab status
- ✅ Link GitLab project
- ✅ Get task merge requests

### Deployments Tests (`deployments_tests.rs`)
- ✅ List deployments
- ✅ Trigger build
- ✅ Get artifacts
- ✅ Trigger deploy

## Test Helpers

### `setup_test_db()`
Setup test database connection và chạy migrations.

### `create_test_app_state(pool)`
Tạo AppState với test configuration.

### `create_test_user(pool, email, password, roles)`
Tạo test user trong database.

### `create_test_admin(pool)`
Tạo test admin user.

### `generate_test_token(user_id)`
Generate JWT token cho test user.

### `create_test_project(pool, created_by, name)`
Tạo test project.

### `create_test_task(pool, project_id, created_by, title)`
Tạo test task.

### `create_test_attempt(pool, task_id, status)`
Tạo test task attempt.

### `cleanup_test_data(pool, user_id, project_id)`
Cleanup test data sau khi test xong.

### `make_request(router, method, path, body, headers)`
Make HTTP request đến test router và return (status, body).

## Best Practices

1. **Always cleanup**: Sử dụng `cleanup_test_data` sau mỗi test
2. **Use helpers**: Sử dụng helper functions thay vì duplicate code
3. **Test isolation**: Mỗi test nên độc lập, không phụ thuộc vào test khác
4. **Assertions**: Luôn assert status code và response structure
5. **Error cases**: Test cả success và error cases

## Notes

- Tất cả tests được mark với `#[ignore]` vì require test database
- Chạy với `--ignored` flag để run tests
- Tests sử dụng real database connection (không mock)
- Mỗi test tự cleanup data sau khi chạy
- Một số tests (Preview, Deployments, GitLab) có thể fail nếu external services chưa được setup
- Admin tests có thể require admin role check implementation

## Test Statistics

**Total Test Files**: 18 files  
**Total Test Cases**: ~85+ test cases  
**Coverage**: All major API endpoints covered

### Breakdown by Category

- **Core APIs**: Authentication, Health, Users, Dashboard (19 tests)
- **Project Management**: Projects, Tasks, Task Attempts (21 tests)
- **Planning**: Sprints, Requirements (12 tests)
- **Review & Activity**: Reviews, Agent Activity (11 tests)
- **Infrastructure**: Settings, Preview, Templates, Admin (16 tests)
- **Integrations**: GitLab, Deployments (7 tests)