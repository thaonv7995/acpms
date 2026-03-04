# UI Flow: Kanban Board & Unified Tasks

The Kanban system is used in both the global "Tasks" view and the project-specific "Tasks" tab. It uses a high-performance 3-panel architecture.

## Screen Overview
- **Left Panel (Kanban)**: Infinite scrolling columns (Todo, In Progress, Review, Done).
- **Middle Panel (Attempt)**: Log streaming (`TaskAttemptPanel`) and follow-up actions (`TodoPanel`).
- **Right Panel (Aux)**: Context-aware sidebar (Preview environment or Git Diff viewer).

## Interactive Elements & Actions

### 1. Board Controls
- **Action: Project Dropdown**
    - **Trigger**: Select a project or "All Projects".
    - **URL Update**: Navigates between `/tasks/projects` and `/tasks/projects/:id`.
    - **Effect**: Filters the entire kanban board.
- **Action: "Add Task" (Column Header)**
    - **Trigger**: Click '+' in any column.
    - **Modal**: Opens `CreateTaskModal`.

### 2. Panel Navigation
- **Action: Click Task Card**
    - **Result**: Opens the middle panel (`TaskPanel`). Navigates to `.../tasks/:taskId`.
- **Action: Select Attempt (Middle Panel)**
    - **Result**: Switches logs and follow-up view to that specific attempt. Navigates to `.../attempts/:attemptId`.

### 3. Aux Views (Layout Modes)
- **Action: "Review Diff" (Header)**
    - **Trigger**: Select "Diff" icon.
    - **URL Param**: Sets `?view=diffs`.
    - **Result**: Right-hand panel displays `DiffsPanel`.
- **Action: "Preview" (Header)**
    - **Trigger**: Select "Preview" icon.
    - **URL Param**: Sets `?view=preview`.
    - **Result**: Right-hand panel displays `PreviewPanelWrapper` (iframe to the instance).

### 4. Keyboard Shortcuts
- **Create Task**: `C`
- **Toggle Layout**: `V` (Cycles between Single, Split-Diff, Split-Preview)
- **Close Panel**: `Esc`
- **Navigate Search**: `/`
