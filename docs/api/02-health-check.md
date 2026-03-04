# Health Check API

API endpoints để kiểm tra health status của server.

## Base Path

Không có prefix (root level)

---

## Endpoints

### 1. GET `/health`

Basic health check endpoint.

#### Request

Không có headers hoặc body yêu cầu.

#### Response

**Status**: `200 OK`

**Body**: JSON (`HealthResponse`)
```json
{
  "status": "healthy",
  "version": "x.y.z",
  "timestamp": "2026-02-07T10:00:00Z",
  "components": {
    "service": {
      "status": "healthy",
      "message": "Service is running"
    }
  }
}
```

#### Usage

Được sử dụng bởi:
- Load balancers
- Monitoring systems
- Kubernetes liveness probes

**Backend Implementation**: `crates/server/src/routes/health.rs::health_check`

---

### 2. GET `/health/ready`

Readiness check - kiểm tra service có sẵn sàng nhận requests không.

#### Request

Không có headers hoặc body yêu cầu.

#### Response

**Status**: `200 OK` nếu service ready, `503 Service Unavailable` nếu chưa ready

**Body**: JSON (`HealthResponse`) với chi tiết `database`, `worker_queue`...

#### Usage

Được sử dụng bởi:
- Kubernetes readiness probes
- Service discovery systems

**Backend Implementation**: `crates/server/src/routes/health.rs::readiness_check`

**Logic**: Kiểm tra database connection và các dependencies khác.

---

### 3. GET `/health/live`

Liveness check - kiểm tra service có đang chạy không.

#### Request

Không có headers hoặc body yêu cầu.

#### Response

**Status**: `200 OK`

**Body**: JSON (`HealthResponse`)

#### Usage

Được sử dụng bởi:
- Kubernetes liveness probes
- Process managers

**Backend Implementation**: `crates/server/src/routes/health.rs::liveness_check`

---

## Notes

- Tất cả health check endpoints đều **không yêu cầu authentication**
- Response format là JSON
- Các endpoints này được expose ở root level, không có `/api/v1` prefix
