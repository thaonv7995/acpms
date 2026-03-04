# UI Interaction Flows

This directory maps the Frontend user interface elements (buttons, forms, pages) to their corresponding Backend API endpoints and business logic flows.

## Page-by-Page Guides

- [**Dashboard**](dashboard.md)
  Overview of stats, projects list, and agent live feeds.
- [**Projects List**](projects-list.md)
  Browsing, searching, and creating new projects.
- [**Project Detail**](project-detail.md)
  Overview, stats, and individual project settings.
- [**Kanban Board & Unified Tasks**](kanban-board.md)
  High-performance task management with split-view diffs and previews.
- [**Agent Stream**](agent-stream.md)
  Real-time global monitoring of all agent activities and logs.
- [**Merge Requests**](merge-requests.md)
  Reviewing and syncing code changes from GitLab.
- [**Task Detail & Agent Control**](task-detail.md)
  Legacy specialized task view and agent initiation dashboard.
- [**Authentication & Settings**](auth-settings.md)
  Login flow and global system integration configurations.
- [**Profile & Administration**](profile-admin.md)
  User identity, password management, and admin user dashboard.

## How to use this guide
Use these documents when you need to trace a user action from the UI button click through the API layer down to the Rust service implementation.
