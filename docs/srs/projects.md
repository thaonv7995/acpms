# SRS: Projects List

## 1. Introduction
The Projects List screen allows users to browse all repositories connected to the system, filter them by status or tech stack, and initiate the creation of new projects.

## 2. Access Control
- **Roles**: All authenticated users.
- **Permissions**: 
  - Standard users can view all public/shared projects.
  - Users with `admin` or `product_owner` roles can Create, Edit, or Delete projects.

## 3. UI Components
- **Search & Filter Bar**: Text search, Status dropdown (Active, Archived), and Tech Stack filters.
- **Project Grid/Table**: Cards showing project name, description, tech stack icons, and last activity.
- **Create Project Modal**: Form for repository URL, name, and initial settings.

## 4. Functional Requirements

### [SRS-PRJ-001] Fetch Projects
- **Trigger**: Screen navigation or filter change.
- **Input**: `search_query`, `status_filter`, `tech_stack_filter`.
- **Output**: Filtered list of projects.
- **System Logic**: Calls `GET /api/v1/projects` with query parameters.
- **Validation**: Handle empty states with a "No Projects Found" message and a "Clear Filters" button.

### [SRS-PRJ-002] Create Project
- **Trigger**: Click "Create Project" button -> Submit Form.
- **Input**: `name`, `repository_url`, `branch`, `description`.
- **Output**: New project entry in the list; success toast.
- **System Logic**: Calls `POST /api/v1/projects`. Validates repository access.
- **Validation**: 
  - Repository URL must be valid.
  - Project name must be unique within the organization.

### [SRS-PRJ-003] Quick Search
- **Trigger**: Typing in the search bar.
- **Input**: Search string.
- **Output**: Instant filtering of the project list.
- **System Logic**: Client-side filtering for immediate feedback, followed by a debounced API call for larger datasets.

### [SRS-PRJ-004] Edit Project Basics
- **Trigger**: Click "Edit" in the project card menu.
- **Input**: Updated name or description.
- **Output**: Updated project card.
- **System Logic**: Calls `PUT /api/v1/projects/:id`.

### [SRS-PRJ-005] Soft Delete Project
- **Trigger**: Click "Delete" -> Confirm in dialog.
- **Input**: `project_id`.
- **Output**: Project removed from view; success toast.
- **System Logic**: Calls `DELETE /api/v1/projects/:id`. Performs a soft delete (DB flag).

## 5. Non-Functional Requirements
- **Responsiveness**: Support for "infinite scroll" or pagination if project count > 50.
- **Security**: "Create Project" button is disabled/hidden for users without write permissions.
