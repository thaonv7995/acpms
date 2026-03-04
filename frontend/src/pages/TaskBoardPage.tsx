import { ProjectTasksPage } from './ProjectTasksPage';

/**
 * TaskBoardPage - Shows all tasks across all projects
 *
 * This page uses ProjectTasksPage with projectId='all' to show all tasks
 * in a unified kanban view with project selector dropdown.
 * 
 * URL: /tasks -> Shows all tasks from all projects
 */
export function TaskBoardPage() {
  // Use ProjectTasksPage but override projectId to 'all'
  // We'll handle this by checking if we're on /tasks route
  return <ProjectTasksPage />;
}
