# Task Initiation & Agent Execution Flow

This document describes the end-to-end technical flow for starting an agent to work on a task.

## Flow Diagram

```mermaid
sequenceDiagram
    participant User
    participant Frontend (UI/React)
    participant Backend (Axum/API)
    participant WorkerPool
    participant Orchestrator
    participant AgentCLI (Process)

    User->>Frontend (UI/React): Click "Start Agent" on Task Detail Page
    Frontend (UI/React)->>Backend (Axum/API): POST /api/v1/tasks/:id/attempts
    Backend (Axum/API)->>Backend (Axum/API): create_task_attempt handler
    Note over Backend (Axum/API): Authenticate & Check Permissions
    Backend (Axum/API)->>DB: Create TaskAttempt (status: queued)
    Backend (Axum/API)->>DB: Update Task (status: in_progress)
    Backend (Axum/API)->>WorkerPool: Submit(AgentJob)
    Backend (Axum/API)-->>Frontend (UI/React): 201 Created (Attempt ID)

    WorkerPool->>Orchestrator: execute_task_with_cancel_review
    Orchestrator->>Orchestrator: Setup Worktree/Environment
    Orchestrator->>Orchestrator: Resolve agent provider from system settings
    Orchestrator->>AgentCLI (Process): Spawn selected CLI (claude-code / openai-codex / gemini-cli)
    AgentCLI (Process)-->>Orchestrator: Executing...
```

## Technical Components

### 1. Frontend Entry
- **File**: `frontend/src/pages/TaskDetailPage.tsx`
- **Action**: `handleStartAgent` calls `createTaskAttempt(taskId)` from `frontend/src/api/taskAttempts.ts`.

### 2. Backend API Endpoint
- **File**: `crates/server/src/routes/task_attempts.rs`
- **Function**: `create_task_attempt`
- **Logic**:
    - **Authentication**: `auth_user: AuthUser` extractor verifies JWT.
    - **Authorization**: `RbacChecker::check_permission` verifies `Permission::ExecuteTask`.
    - **Persistence**: `TaskAttemptService` creates a record in the `task_attempts` table.
    - **Task Update**: `TaskService` set task status to `InProgress`.

### 3. Job Submission
- **Worker Pool**: The job is submitted to `state.worker_pool.submit(job)`.
- **Job Definition**: `AgentJob` includes attempt ID, task ID, repo path, and project settings (timeout, retry).

### 4. Orchestration
- **File**: `crates/executors/src/orchestrator.rs`
- **Method**: `execute_task_with_cancel_review`
- **Environment**: Sets up a temporary worktree using `WorktreeManager`.
- **Execution**: Spawns the selected agent CLI process (provider chosen in Settings) and redirects its pipes for log capturing.
