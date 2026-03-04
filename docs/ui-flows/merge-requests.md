# UI Flow: Merge Requests Dashboard

This screen manages the integration between agent-generated code and the GitLab VCS.

## Screen Overview
- **Stats Row**: Summary of Open, Pending Review, Merged, and AI-Generated MR count.
- **MR List**: Tabbed view of all Merge Requests associated with linked projects.
- **Primary Action**: "Sync with GitLab" button.

## Interactive Elements & Actions

### 1. Synchronization
- **Action: "Sync with GitLab" Button**
    - **Trigger**: User clicks button top right.
    - **Effect**: Calls `syncWithGitLab()` in `useMergeRequests` hook.
    - **Backend**: Current backend không có endpoint manual sync riêng; dữ liệu MR được lấy qua `GET /api/v1/tasks/:id/gitlab/merge_requests` và cập nhật qua webhook `POST /api/v1/webhooks/gitlab`.

### 2. Navigation & Filtering
- **Action: Status Tabs**
    - **Trigger**: Click All, Open, Pending, or Merged.
    - **Effect**: Filters the MR list client-side.
- **Action: Click MR Card**
    - **Result**: Navigates to the external GitLab MR page (current implementation) or a specialized internal review view.

### 3. Search
- **Action: Search Input**
    - **Trigger**: Type in search box.
    - **Effect**: Filters result set by MR title, project name, or author.
    - **Hook**: Uses `debouncedSearch` to avoid rapid API requests if server-side filtering is active.
