import { Task } from '../../api/tasks';
import type { TaskContext } from '../../api/taskContexts';
import { TaskDescriptionCard } from './TaskDescriptionCard';
import { TaskActivityFeed } from './TaskActivityFeed';
import { PreviewSection } from './PreviewSection';
import { TaskDocumentPreview } from './TaskDocumentPreview';
import { isTaskDocumentPreview } from '../../lib/taskDocuments';

interface TaskStatusContentProps {
    task: Task;
    taskContexts: TaskContext[];
    normalizedStatus: string;
    artifactAttemptId?: string;
    previewMetadata?: Record<string, unknown>;
}

/**
 * Renders different content sections based on task status.
 * Note: Attempts and Code Changes are now shown in the View Logs drawer.
 */
export function TaskStatusContent({
    task,
    taskContexts,
    normalizedStatus,
    artifactAttemptId,
    previewMetadata,
}: TaskStatusContentProps) {
    const metadata = previewMetadata ?? task.metadata;
    const showDocumentPreview = isTaskDocumentPreview(task.task_type, metadata);

    // Common sections that appear in all statuses
    const descriptionSection = <TaskDescriptionCard description={task.description} />;

    switch (normalizedStatus) {
        case 'in_review':
            return (
                <div className="flex flex-col gap-6">
                    {descriptionSection}
                    {showDocumentPreview ? (
                        <TaskDocumentPreview
                            task={task}
                            taskContexts={taskContexts}
                            metadata={metadata}
                            isReviewMode
                        />
                    ) : (
                        <PreviewSection
                            previewUrl={metadata?.preview_url as string}
                            appDownloadUrl={metadata?.app_download_url as string}
                            appDownloads={metadata?.app_downloads as Array<Record<string, unknown>>}
                            artifactAttemptId={artifactAttemptId}
                            previewTarget={metadata?.preview_target as string}
                            deploymentStatus={metadata?.deployment_status as string}
                            deploymentError={metadata?.deployment_error as string}
                            appVersion={metadata?.app_version as string}
                        />
                    )}
                    <TaskActivityFeed />
                </div>
            );

        case 'done':
            return (
                <div className="flex flex-col gap-6">
                    {descriptionSection}
                    {showDocumentPreview ? (
                        <TaskDocumentPreview
                            task={task}
                            taskContexts={taskContexts}
                            metadata={metadata}
                        />
                    ) : (
                        <PreviewSection
                            previewUrl={metadata?.preview_url as string}
                            appDownloadUrl={metadata?.app_download_url as string}
                            appDownloads={metadata?.app_downloads as Array<Record<string, unknown>>}
                            artifactAttemptId={artifactAttemptId}
                            previewTarget={metadata?.preview_target as string}
                            deploymentStatus={metadata?.deployment_status as string}
                            deploymentError={metadata?.deployment_error as string}
                            appVersion={metadata?.app_version as string}
                            isCompleted
                        />
                    )}
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
