# UI Flow: Projects List

The Projects page is the main entry point for managing repositories and initializing new agentic environments.

## Screen Overview
- **Search Bar**: Real-time filtering by project name/description.
- **Filter Dropdowns**: Status and Tech Stack filters.
- **Project Grid**: Cards representing each project with status and health metrics.
- **Primary Action**: "Create New Project" button.

## Interactive Elements & Actions

### 1. Global Search & Filters
- **Action**: Type in Search Input.
- **Effect**: Filters the `filteredProjects` list client-side (backed by `useProjects` hook).
- **Action**: Select Status or Tech Stack option.
- **Effect**: Refines the project grid based on metadata properties.

### 2. Create New Project
- **Trigger**: Click "Create New Project" button (top right).
- **Modal**: Opens `CreateProjectModal`.
- **Options**:
    - **Import**: Enter GitLab URL -> Calls `POST /api/v1/projects/import`.
    - **From Scratch**: Enter name -> Calls `POST /api/v1/projects` with `create_from_scratch: true`.
- **Backend Flow**: Triggers the [Project Import](../feature-flows/project-import.md) or [Project Creation](../feature-flows/project-creation.md) flows.

### 3. Project Card Actions
- **Action: Click Card**
    - **Result**: Navigates to `/projects/:id`.
- **Action: "Edit" (Dropdown)**
    - **Modal**: Opens `EditProjectModal`.
    - **Submission**: Calls `PUT /api/v1/projects/:id`.
    - **Backend**: `crates/server/src/routes/projects.rs:update_project`.
- **Action: "Delete" (Dropdown)**
    - **Modal**: Opens `ConfirmModal`.
    - **Submission**: Calls `DELETE /api/v1/projects/:id`.
    - **Backend**: `crates/server/src/routes/projects.rs:delete_project`.
