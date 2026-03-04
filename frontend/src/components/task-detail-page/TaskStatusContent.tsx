import { Task } from '../../api/tasks';
import { TaskDescriptionCard } from './TaskDescriptionCard';
import { TaskActivityFeed } from './TaskActivityFeed';
import { PreviewSection } from './PreviewSection';

interface TaskStatusContentProps {
    task: Task;
    normalizedStatus: string;
}

/**
 * Renders different content sections based on task status.
 * Note: Attempts and Code Changes are now shown in the View Logs drawer.
 */
export function TaskStatusContent({
    task,
    normalizedStatus,
}: TaskStatusContentProps) {
    // Common sections that appear in all statuses
    const descriptionSection = <TaskDescriptionCard description={task.description} />;

    switch (normalizedStatus) {
        case 'in_review':
            return (
                <div className="flex flex-col gap-6">
                    {descriptionSection}
                    {/* Preview section if available */}
                    <PreviewSection
                        previewUrl={task.metadata?.preview_url as string}
                        appDownloadUrl={task.metadata?.app_download_url as string}
                        appDownloads={task.metadata?.app_downloads as Array<Record<string, unknown>>}
                        previewTarget={task.metadata?.preview_target as string}
                        deploymentStatus={task.metadata?.deployment_status as string}
                        deploymentError={task.metadata?.deployment_error as string}
                        appVersion={task.metadata?.app_version as string}
                    />
                    <TaskActivityFeed />
                </div>
            );

        case 'done':
            return (
                <div className="flex flex-col gap-6">
                    {descriptionSection}
                    {/* Show final deployment/preview */}
                    <PreviewSection
                        previewUrl={task.metadata?.preview_url as string}
                        appDownloadUrl={task.metadata?.app_download_url as string}
                        appDownloads={task.metadata?.app_downloads as Array<Record<string, unknown>>}
                        previewTarget={task.metadata?.preview_target as string}
                        deploymentStatus={task.metadata?.deployment_status as string}
                        deploymentError={task.metadata?.deployment_error as string}
                        appVersion={task.metadata?.app_version as string}
                        isCompleted
                    />
                    <TaskActivityFeed />
                </div>
            );

        default:
            // todo, in_progress, and other statuses
            return (
                <div className="flex flex-col gap-6">
                    {descriptionSection}
                    <TaskActivityFeed />
                </div>
            );
    }
}
