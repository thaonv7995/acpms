import type { LayoutMode } from '../../components/layout/TasksLayout';
import type { KanbanTask } from '../../types/project';
import type { TaskAttempt } from '../../types/task-attempt';
import { NewCardHeader } from '@/components/ui/new-card';
import { AttemptHeaderActions } from '@/components/panels/AttemptHeaderActions';
import {
  Breadcrumb,
  BreadcrumbList,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';

interface ProjectTasksHeaderProps {
  selectedTask: KanbanTask;
  selectedAttempt: TaskAttempt | null;
  mode: LayoutMode;
  isTaskView: boolean;
  onModeChange: (mode: LayoutMode) => void;
  onBackToTask: () => void;
  onClose: () => void;
  previewModeDisabled?: boolean;
  previewModeDisabledReason?: string;
  downloadArtifactUrl?: string;
  downloadArtifactLabel?: string;
  downloadDisabled?: boolean;
  downloadDisabledReason?: string;
  onDownloadArtifact?: () => void;
  onCreateAttempt?: () => void;
  onOpenGitActions?: () => void;
  onDeleteTask?: () => void;
}

/**
 * Truncate title for breadcrumbs
 */
function truncateTitle(title: string | undefined, maxLength = 20) {
  if (!title) return 'Task';
  if (title.length <= maxLength) return title;

  const truncated = title.substring(0, maxLength);
  const lastSpace = truncated.lastIndexOf(' ');

  return lastSpace > 0
    ? `${truncated.substring(0, lastSpace)}...`
    : `${truncated}...`;
}

/**
 * Header component for ProjectTasks page with breadcrumbs and mode toggles
 */
export function ProjectTasksHeader({
  selectedTask,
  selectedAttempt,
  mode,
  isTaskView,
  onModeChange,
  onBackToTask,
  onClose,
  previewModeDisabled = false,
  previewModeDisabledReason,
  downloadArtifactUrl,
  downloadArtifactLabel,
  downloadDisabled = false,
  downloadDisabledReason,
  onDownloadArtifact,
  onCreateAttempt,
  onOpenGitActions,
  onDeleteTask,
}: ProjectTasksHeaderProps) {
  // Ensure we have task data before rendering
  if (!selectedTask) {
    return null;
  }

  const taskTitle = selectedTask.title || 'Untitled Task';
  const attemptBranch = selectedAttempt?.branch || 'Attempt';

  return (
    <NewCardHeader
      className="shrink-0 sticky top-0 z-20 bg-background border-b border-border"
      actions={
        !isTaskView && selectedAttempt ? (
          <AttemptHeaderActions
            mode={mode}
            onModeChange={onModeChange}
            task={selectedTask}
            attempt={selectedAttempt}
            onClose={onClose}
            previewDisabled={previewModeDisabled}
            previewDisabledReason={previewModeDisabledReason}
            downloadArtifactUrl={downloadArtifactUrl}
            downloadArtifactLabel={downloadArtifactLabel}
            downloadDisabled={downloadDisabled}
            downloadDisabledReason={downloadDisabledReason}
            onDownloadArtifact={onDownloadArtifact}
            onCreateAttempt={onCreateAttempt}
            onOpenGitActions={onOpenGitActions}
            onDeleteTask={onDeleteTask}
          />
        ) : (
          <AttemptHeaderActions
            task={selectedTask}
            attempt={null}
            onClose={onClose}
            onCreateAttempt={onCreateAttempt}
            onDeleteTask={onDeleteTask}
          />
        )
      }
    >
      <div className="mx-auto w-full">
        <Breadcrumb>
          <BreadcrumbList>
            <BreadcrumbItem>
              {isTaskView ? (
                <BreadcrumbPage>
                  {truncateTitle(taskTitle)}
                </BreadcrumbPage>
              ) : (
                <BreadcrumbLink
                  className="cursor-pointer hover:underline"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onBackToTask();
                  }}
                  href="#"
                >
                  {truncateTitle(taskTitle)}
                </BreadcrumbLink>
              )}
            </BreadcrumbItem>
            {!isTaskView && (
              <>
                <BreadcrumbSeparator />
                <BreadcrumbItem>
                  <BreadcrumbPage>
                    {attemptBranch}
                  </BreadcrumbPage>
                </BreadcrumbItem>
              </>
            )}
          </BreadcrumbList>
        </Breadcrumb>
      </div>
    </NewCardHeader>
  );
}
