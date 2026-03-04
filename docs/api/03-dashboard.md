# Dashboard API

API endpoint để lấy dữ liệu dashboard tổng hợp.

## Base Path

`/api/v1/dashboard`

---

## Endpoints

### 1. GET `/api/v1/dashboard`

Lấy dữ liệu dashboard tổng hợp bao gồm stats, projects, agent logs, và human tasks.

#### Request

**Headers**:
```
Authorization: Bearer <access_token>
```

**Query Parameters**: Không có

#### Response

**Status**: `200 OK`

**Body**:
```json
{
  "success": true,
  "code": "0000",
  "message": "Dashboard data retrieved successfully",
  "data": {
    "stats": {
      "activeProjects": {
        "count": 5,
        "trend": "+2 this week"
      },
      "agentsOnline": {
        "online": 2,
        "total": 7
      },
      "systemLoad": {
        "percentage": 32,
        "status": "medium"
      },
      "pendingPRs": {
        "count": 3,
        "requires_review": true
      }
    },
    "projects": [
      {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "Project Name",
        "subtitle": "Project description",
        "status": "building",
        "progress": 65,
        "agents": [
          {
            "id": "1",
            "initial": "A",
            "color": "blue"
          }
        ]
      }
    ],
    "agentLogs": [
      {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "timestamp": "2026-01-13T10:00:00Z",
        "agentName": "Agent",
        "agentColor": "purple",
        "message": "Task completed successfully",
        "highlight": null
      }
    ],
    "humanTasks": [
      {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "type": "review",
        "title": "Review task changes",
        "description": "Please review the changes",
        "createdAt": "2026-01-13T10:00:00Z",
        "assignee": {
          "id": "550e8400-e29b-41d4-a716-446655440000",
          "avatar": null
        }
      }
    ]
  },
  "metadata": null,
  "error": null
}
```

**Fields**:

**stats**:
- `activeProjects.count` (number): Tổng số active projects
- `activeProjects.trend` (string): Trend của active projects (ví dụ: "+2 this week")
- `agentsOnline.online` (number): Số agents đang online (running attempts)
- `agentsOnline.total` (number): Tổng số agents
- `systemLoad.percentage` (number): System load percentage (0-100)
- `systemLoad.status` (string): "low" | "medium" | "high"
- `pendingPRs.count` (number): Số pending PRs (tasks với status "in_review")
- `pendingPRs.requires_review` (boolean): Có tasks cần review không

**projects** (array, max 5 items):
- `id` (UUID): Project ID
- `name` (string): Project name
- `subtitle` (string): Project description
- `status` (string): "building" | "testing" | "deploying" | "completed"
- `progress` (number): Progress percentage (0-100), tính từ completed tasks
- `agents` (array): List of agent avatars

**agentLogs** (array, max 10 items):
- `id` (UUID): Log ID
- `timestamp` (datetime): Log timestamp
- `agentName` (string): Agent name
- `agentColor` (string): Agent color
- `message` (string): Log message (truncated to 50 chars)
- `highlight` (string | null): Highlight text if any

**humanTasks** (array, max 5 items):
- `id` (UUID): Task ID
- `type` (string): "blocker" | "approval" | "qa" | "review"
- `title` (string): Task title
- `description` (string): Task description
- `createdAt` (datetime): Task creation timestamp
- `assignee` (object | null): Assigned user avatar

#### Error Responses

**403 Forbidden** - Không có quyền:
```json
{
  "success": false,
  "code": "4030",
  "message": "Forbidden",
  "data": null,
  "error": {
    "details": "Access denied",
    "trace_id": "abc123-def456"
  }
}
```

#### Frontend Usage

**File**: `frontend/src/hooks/useDashboard.ts`

**Example**:
```typescript
import { useQuery } from '@tanstack/react-query';
import { getDashboardData } from '@/api/dashboard';

export function useDashboard() {
  return useQuery({
    queryKey: ['dashboard'],
    queryFn: getDashboardData,
    refetchInterval: 30000, // Refetch every 30 seconds
  });
}
```

**Màn hình**: Dashboard Page (`/dashboard`)

**Components**:
- `StatCard.tsx`: Hiển thị stats (active projects, agents online, system load, pending PRs)
- `ProjectsTable.tsx`: Hiển thị danh sách projects với progress bar
- `AgentFeed.tsx`: Hiển thị agent logs feed
- `HumanTaskCard.tsx`: Hiển thị human tasks cần review

**Nhiệm vụ**: 
- Load và hiển thị dashboard data
- Auto-refresh mỗi 30 giây
- Hiển thị stats, projects, agent logs, và human tasks

#### Backend Implementation

**File**: `crates/server/src/routes/dashboard.rs::get_dashboard`

**Service**: `crates/services/src/dashboard.rs::DashboardService`

**Logic**:
1. **Stats**:
   - Active Projects: Count từ `projects` table
   - Agents Online: Count `task_attempts` với `status = 'running'`
   - System Load: Mocked (32%, "medium")
   - Pending PRs: Count `tasks` với `status = 'in_review'`

2. **Projects** (top 5 most recent):
   - Query projects với calculated progress
   - Progress = `(completed_tasks / total_tasks) * 100`
   - Completed tasks = tasks với `status IN ('done', 'archived')`
   - Order by `updated_at DESC`

3. **Agent Logs** (recent 10):
   - Query từ `agent_logs` table
   - Order by `created_at DESC`
   - Limit 10

4. **Human Tasks** (recent 5):
   - Query tasks với `status != 'done'`
   - Filter: `assigned_to = user_id OR assigned_to IS NULL`
   - Order by `created_at DESC`
   - Limit 5

**Database Queries**:
- Uses CTE (Common Table Expression) để calculate progress
- JOIN với `task_stats` để tính progress percentage

---

## Data Calculation Details

### Project Progress Calculation

```sql
WITH task_stats AS (
    SELECT 
        project_id,
        COUNT(*) as total,
        COUNT(*) FILTER (WHERE status::text IN ('done', 'archived')) as completed
    FROM tasks
    GROUP BY project_id
)
SELECT 
    p.id, 
    p.name, 
    p.description, 
    CASE
        WHEN ts.total IS NULL OR ts.total = 0 THEN 0
        ELSE ROUND((ts.completed::numeric / ts.total::numeric) * 100)::bigint
    END as progress
FROM projects p
LEFT JOIN task_stats ts ON ts.project_id = p.id
ORDER BY p.updated_at DESC
LIMIT 5
```

### Agent Online Count

```sql
SELECT COUNT(*) FROM task_attempts WHERE status = 'running'
```

### Pending PRs Count

```sql
SELECT COUNT(*) FROM tasks WHERE status = 'in_review'
```

---

## Performance Notes

- Dashboard query được optimize với indexes trên:
  - `projects.updated_at`
  - `task_attempts.status`
  - `tasks.status`
  - `tasks.assigned_to`
- Limits được áp dụng để tránh load quá nhiều data:
  - Projects: 5 items
  - Agent Logs: 10 items
  - Human Tasks: 5 items
