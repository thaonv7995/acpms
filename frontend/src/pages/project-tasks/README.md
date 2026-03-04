# ProjectTasksPage - Modular Architecture

## Overview

Rewritten from 277 lines to **681 total lines** (main page: 285 LOC) following vibe-kanban reference pattern with full component integration.

## File Structure

```
project-tasks/
├── index.ts                              # Barrel exports
├── use-project-tasks-navigation.ts       # Navigation logic (78 LOC)
├── use-attempt-data.ts                   # Attempt data & redirects (88 LOC)
├── use-keyboard-shortcuts.ts             # Keyboard event handling (38 LOC)
├── project-tasks-header.tsx              # Header with breadcrumbs & toggles (110 LOC)
├── preview-panel-wrapper.tsx             # Dev server integration (27 LOC)
└── README.md                             # This file
```

## Key Features Implemented

### 1. **Three-Panel Layout System**
- **Kanban Board** - Task columns with drag & drop
- **Attempt Panel** - VirtualizedLogList with conversation history
- **Aux Panel** - Preview or Diffs based on mode

### 2. **Panel Modes**
```typescript
mode = null      → Kanban | Attempt (logs)
mode = 'preview' → Attempt | Preview (dev server)
mode = 'diffs'   → Attempt | Diffs (code changes)
```

### 3. **Component Integration**

#### Phase Components Used:
- ✅ **VirtualizedLogList** (Phase 4) - Performance-optimized log rendering
- ✅ **AttemptSwitcher** (Phase 2.5) - Navigate between attempts
- ✅ **TaskFollowUpSection** (Phase 6) - Via TaskAttemptPanel
- ✅ **useConversationHistory** (Phase 7) - WebSocket log streaming
- ✅ **PreviewPanel** (Phase 5.5) - Dev server preview
- ✅ **DiffsPanel** (Phase 5.6) - Code diff viewer

#### Provider Integration:
- ✅ **RetryUiProvider** - Retry UI state management
- ✅ **ReviewProvider** - Code review context
- ⚠️ **ExecutionProcessesProvider** - TODO: Add when available
- ⚠️ **GitOperationsProvider** - TODO: Add when available

### 4. **URL Structure**

```
/projects/:projectId/tasks
  → Kanban only

/projects/:projectId/tasks/:taskId
  → Kanban | Task details

/projects/:projectId/tasks/:taskId/attempts/:attemptId
  → Kanban | Attempt logs

?view=preview
  → Attempt | Preview panel

?view=diffs
  → Attempt | Diffs panel
```

### 5. **Keyboard Shortcuts**

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + K` | Create new task |
| `Escape` | Close panel |
| `Cmd/Ctrl + Enter` | Cycle view mode (Logs → Preview → Diffs) |

### 6. **Navigation Features**

- ✅ `/attempts/latest` auto-redirect to most recent attempt
- ✅ Task click → auto-navigate to latest attempt
- ✅ Search params preservation across navigation
- ✅ Breadcrumb navigation with truncation
- ✅ Back to task from attempt view

### 7. **Responsive Design**

- Desktop: 3-panel split with resizable panels
- Mobile: Single panel with mode-based switching
- Panel collapse/expand support

## Comparison with vibe-kanban Reference

| Feature | vibe-kanban | acpms-project | Status |
|---------|-------------|---------------|--------|
| TasksLayout 3-panel | ✅ | ✅ | Complete |
| VirtualizedLogList | ✅ | ✅ | Complete |
| AttemptSwitcher | ✅ | ✅ | Complete |
| useConversationHistory | ✅ | ✅ | Complete |
| PreviewPanel | ✅ | ✅ | Complete |
| DiffsPanel | ✅ | ✅ | Complete |
| RetryUiProvider | ✅ | ✅ | Complete |
| ReviewProvider | ✅ | ✅ | Complete |
| ExecutionProcessesProvider | ✅ | ⚠️ | Pending |
| GitOperationsProvider | ✅ | ⚠️ | Pending |
| Keyboard shortcuts | ✅ | ✅ | Complete |
| TodoPanel | ✅ | ⚠️ | In TaskAttemptPanel |

## Architecture Decisions

### 1. **Modularization Strategy**
- Extracted hooks for single-responsibility
- Separated navigation, data fetching, and UI logic
- Header component for complex breadcrumb/mode UI

### 2. **State Management**
- URL-driven state (taskId, attemptId, mode)
- React Query for server state
- Local state for modals only

### 3. **Performance**
- VirtualizedLogList for large log datasets
- Memoization for expensive computations
- WebSocket streaming for real-time updates

### 4. **Type Safety**
- Strict TypeScript throughout
- API response mapping to domain types
- Proper null/undefined handling

## Usage Example

```typescript
import { ProjectTasksPage } from './pages/ProjectTasksPage';

// In router
<Route path="/projects/:projectId/tasks" element={<ProjectTasksPage />} />
<Route path="/projects/:projectId/tasks/:taskId" element={<ProjectTasksPage />} />
<Route path="/projects/:projectId/tasks/:taskId/attempts/:attemptId" element={<ProjectTasksPage />} />
```

## TODOs

- [ ] Add ExecutionProcessesProvider when available
- [ ] Add GitOperationsProvider for git operations
- [ ] Integrate TodoPanel in TaskAttemptPanel
- [ ] Add error boundaries for panel crashes
- [ ] Add loading skeletons for better UX
- [ ] Add tests for navigation hooks
- [ ] Add analytics tracking for mode switches

## Related Files

- `/components/layout/TasksLayout.tsx` - 3-panel layout logic
- `/components/panels/TaskAttemptPanel.tsx` - Attempt logs with VirtualizedLogList
- `/components/panels/AttemptSwitcher.tsx` - Attempt navigation dropdown
- `/components/logs/VirtualizedLogList.tsx` - Performance-optimized log rendering
- `/hooks/useConversationHistory.ts` - WebSocket log streaming
