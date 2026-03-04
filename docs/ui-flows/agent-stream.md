# UI Flow: Agent Stream

The Agent Stream is a high-density monitoring screen for real-time tracking of all active and historical agent attempts across the entire platform.

## Screen Overview
- **Status Header**: Filters (Active, Completed, All) and real-time status counters (Running, Queued).
- **Attempt Cards**: Miniature cards showing the progress, task title, and project for each attempt. Supporting Grid and List view modes.
- **Terminal Console**: A large-scale log viewer with search/filtering capabilities and attempt-specific locking.

## Interactive Elements & Actions

### 1. View Customization
- **Action: Filter Tabs**
    - **Trigger**: Click "Active", "Completed", or "All".
    - **Effect**: Filters the `statuses` array in `useAgentLogs` hook.
- **Action: View Toggle**
    - **Trigger**: Click List/Grid icon.
    - **Effect**: Switches the rendering component for the attempt list.

### 2. Attempt Interaction
- **Action: Click Attempt Card**
    - **Result**: "Locks" the terminal to that specific attempt. Calls `filterByAttempt(id)` in the hook.
    - **Backend**: Filters the WebSocket/Log stream to only show lines matching the `attempt_id`.
- **Action: "Review" (On Card)**
    - **Trigger**: Shown when an attempt is successful but the task is `InReview`.
    - **Modal**: Opens `ReviewChangesModal` (Full-screen diff).
    - **Flow**: Connects to the [GitOps Review flow](../feature-flows/review-approval.md).

### 3. Log Console
- **Action: Search Bar**
    - **Trigger**: Type in search box.
    - **Effect**: Filters log lines matching the regex/string locally in the `VirtualLogList`.
- **Action: "Refresh" (Top Right)**
    - **Trigger**: Click icon.
    - **Effect**: Triggers `refetch()` to sync all current attempt statuses from `GET /api/v1/agent-activity/status`.
