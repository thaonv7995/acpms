import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { LoginPage } from './pages/LoginPage';
import { RegisterPage } from './pages/RegisterPage';
import { DashboardPage } from './pages/DashboardPage';
import { ProjectsPage } from './pages/ProjectsPage';
import { ProjectDetailPage } from './pages/ProjectDetailPage';
// TaskBoardPage removed - replaced by ProjectTasksPage
import { AgentStreamPage } from './pages/AgentStreamPage';
import { SettingsPage } from './pages/SettingsPage';
import { UserManagementPage } from './pages/UserManagementPage';
import { MergeRequestPage } from './pages/MergeRequestPage';
import { ProjectTasksPage } from './pages/ProjectTasksPage';
import { TaskDetailPage } from './pages/TaskDetailPage';
import { ProfilePage } from './pages/ProfilePage';
import { ProtectedRoute } from './components/routing/ProtectedRoute';
import { isAuthenticated } from './api/auth';

function App() {
  return (
    <BrowserRouter
      future={{
        v7_startTransition: true,
        v7_relativeSplatPath: true,
      }}
    >
      <Routes>
        {/* Public routes */}
        <Route
          path="/login"
          element={isAuthenticated() ? <Navigate to="/dashboard" replace /> : <LoginPage />}
        />
        <Route
          path="/register"
          element={isAuthenticated() ? <Navigate to="/dashboard" replace /> : <RegisterPage />}
        />

        {/* Protected routes */}
        <Route
          path="/dashboard"
          element={
            <ProtectedRoute>
              <DashboardPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/projects"
          element={
            <ProtectedRoute>
              <ProjectsPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/projects/:id"
          element={
            <ProtectedRoute>
              <ProjectDetailPage />
            </ProtectedRoute>
          }
        />

        {/* Task detail page - full page view */}
        <Route
          path="/projects/:projectId/task/:taskId"
          element={
            <ProtectedRoute>
              <TaskDetailPage />
            </ProtectedRoute>
          }
        />

        {/* Tasks page with split panel (vibe-kanban pattern) */}
        {/* /tasks - Redirects to /tasks/projects */}
        <Route
          path="/tasks"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        {/* /tasks/projects - Shows all tasks from all projects */}
        <Route
          path="/tasks/projects"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        {/* /tasks/projects/:projectId - Shows tasks for a specific project */}
        <Route
          path="/tasks/projects/:projectId"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        {/* /tasks/projects/:projectId/:taskId - Task detail panel */}
        <Route
          path="/tasks/projects/:projectId/:taskId"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        {/* /tasks/projects/:projectId/:taskId/attempts/:attemptId - Attempt detail with logs */}
        <Route
          path="/tasks/projects/:projectId/:taskId/attempts/:attemptId"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        
        {/* Legacy routes - redirect to new structure */}
        <Route
          path="/projects/:projectId/tasks"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/projects/:projectId/tasks/:taskId"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/projects/:projectId/tasks/:taskId/attempts/:attemptId"
          element={
            <ProtectedRoute>
              <ProjectTasksPage />
            </ProtectedRoute>
          }
        />
        {/* Task detail full page view */}
        <Route
          path="/tasks/:taskId"
          element={
            <ProtectedRoute>
              <TaskDetailPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/merge-requests"
          element={
            <ProtectedRoute>
              <MergeRequestPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/agent-logs"
          element={
            <ProtectedRoute>
              <AgentStreamPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/users"
          element={
            <ProtectedRoute requireAdmin>
              <UserManagementPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/settings"
          element={
            <ProtectedRoute requireAdmin>
              <SettingsPage />
            </ProtectedRoute>
          }
        />
        <Route
          path="/profile"
          element={
            <ProtectedRoute>
              <ProfilePage />
            </ProtectedRoute>
          }
        />

        {/* Default redirect */}
        <Route
          path="/"
          element={<Navigate to={isAuthenticated() ? "/dashboard" : "/login"} replace />}
        />
        <Route
          path="*"
          element={<Navigate to={isAuthenticated() ? "/dashboard" : "/login"} replace />}
        />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
