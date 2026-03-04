# UI Flow: Task Detail & Agent Control

This screen is where users interact directly with agents and review their work.

## Screen Overview
- **Header**: Task title, status badge, and control buttons.
- **Main Area**: Task description and status-specific content (e.g., Progress bars, Review prompts).
- **Sidebar**: Metadata (Priority, Type, Timestamps) and Status selector.

## Interactive Elements & Actions

### 1. Agent Controls
- **Action: "Start Agent" Button**
    - **Modal**: Opens `ConfigureAgentModal`.
    - **Action**: User confirms configuration and clicks "Start".
    - **Submission**: Calls `POST /api/v1/tasks/:id/attempts`.
    - **Backend Flow**: Triggers the [Task Initiation](../feature-flows/task-execution.md) flow.
- **Action: "View Attempts" Button**
    - **Modal**: Opens `ViewLogsModal` drawer.
    - **Effect**: Connects to [WebSocket Stream](../feature-flows/log-streaming.md) for the latest attempt.

### 2. Review Workflow
- **Action: "Review Changes" Button**
    - **Visibility**: Only shown when task status is `InReview` and a successful attempt exists.
    - **Modal**: Opens `DiffViewerModal`.
    - **Action**: User reviews the `git diff`.
- **Action: "Approve" (Inside Diff Modal)**
    - **Submission**: Calls `POST /api/v1/attempts/:id/approve`.
    - **Backend Flow**: Triggers [GitOps Sync](../feature-flows/review-approval.md).
    - **Result**: Task status changes to `Done`.

### 3. Metadata Management
- **Action: Status Dropdown**
    - **Trigger**: Change status in the sidebar (e.g., move back to `Todo`).
    - **Submission**: Calls `PATCH /api/v1/tasks/:id` or specialized update route.
- **Action: Priority/Type Selection**
    - **Submission**: Updates task metadata via `PUT /api/v1/tasks/:id`.
