/**
 * URL path helpers for navigation
 * Centralizes URL construction for consistent routing
 */

export const paths = {
  // Project tasks kanban view
  tasks: (projectId: string) => `/projects/${projectId}/tasks`,

  // Task detail view (shows TaskPanel with attempts list)
  task: (projectId: string, taskId: string) => `/projects/${projectId}/tasks/${taskId}`,

  // Specific attempt view (shows AttemptPanel with logs)
  attempt: (projectId: string, taskId: string, attemptId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/${attemptId}`,

  // Latest attempt redirect (auto-redirects to actual latest attemptId)
  latestAttempt: (projectId: string, taskId: string) =>
    `/projects/${projectId}/tasks/${taskId}/attempts/latest`,
};
