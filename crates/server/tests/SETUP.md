# Test Setup Guide

## Prerequisites

1. **PostgreSQL Database** - Cần PostgreSQL đang chạy
2. **Test Database** - Tạo test database `acpms_test`
3. **Environment Variables** - Set các env vars cần thiết

## Quick Start

### 1. Setup Database

```bash
# Tạo test database
createdb acpms_test

# Hoặc với psql
psql -U postgres -c "CREATE DATABASE acpms_test;"
```

### 2. Set Environment Variables

```bash
PGPORT=$(docker compose port postgres 5432 | awk -F: '/127\.0\.0\.1/ {print $NF; exit}')
export DATABASE_URL="postgresql://acpms_user:acpms_password@127.0.0.1:${PGPORT}/acpms_test"
export ENCRYPTION_KEY="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
export JWT_SECRET="test-jwt-secret-key-for-testing-only"

# Optional: S3 configuration (if testing storage features)
export S3_ENDPOINT="http://localhost:9000"
export S3_PUBLIC_ENDPOINT="http://localhost:9000"
export S3_ACCESS_KEY="admin"
export S3_SECRET_KEY="adminpassword123"
export S3_REGION="us-east-1"
export S3_BUCKET_NAME="acpms-media"

# Optional: GitLab OAuth (if testing GitLab integration)
export GITLAB_CLIENT_ID="test_client_id"
export GITLAB_CLIENT_SECRET="test_client_secret"
export GITLAB_REDIRECT_URI="http://localhost:3000/callback"
```

### 3. Prepare SQLx Query Cache (Optional but Recommended)

Nếu bạn muốn compile mà không cần database connection:

```bash
# Set DATABASE_URL và chạy
PGPORT=$(docker compose port postgres 5432 | awk -F: '/127\.0\.0\.1/ {print $NF; exit}')
export DATABASE_URL="postgresql://acpms_user:acpms_password@127.0.0.1:${PGPORT}/acpms_test"
cargo sqlx prepare --workspace
```

Sau đó có thể compile với:
```bash
export SQLX_OFFLINE=true
cargo test --package acpms-server -- --ignored
```

### 4. Run Tests

```bash
cd acpms-project

# Chạy tất cả tests (bao gồm ignored)
cargo test --package acpms-server -- --ignored

# Chạy với output
cargo test --package acpms-server -- --ignored --nocapture

# Chạy test cụ thể
cargo test --package acpms-server -- --ignored test_health_check

# Chạy test module cụ thể
cargo test --package acpms-server -- --ignored health_tests::

# Hoặc sử dụng script
./crates/server/tests/run_tests.sh
```

## Troubleshooting

### Error: password authentication failed

- Kiểm tra PostgreSQL đang chạy: `pg_isready`
- Kiểm tra password trong `DATABASE_URL` có đúng không
- Thử với user khác trên published port hiện tại của Docker Postgres

### Error: database does not exist

```bash
createdb acpms_test
```

### Error: SQLX_OFFLINE=true but there is no cached data

Chạy `cargo sqlx prepare` hoặc unset `SQLX_OFFLINE`:

```bash
unset SQLX_OFFLINE
cargo test --package acpms-server -- --ignored
```

### Error: StorageService initialization failed

Nếu không test storage features, có thể skip bằng cách comment out StorageService trong helpers.rs hoặc setup MinIO/S3.

## Notes

- Tất cả tests được mark với `#[ignore]` vì require test database
- Tests sử dụng real database connection (không mock)
- Mỗi test tự cleanup data sau khi chạy
- Một số tests (Preview, Deployments, GitLab) có thể fail nếu external services chưa được setup
