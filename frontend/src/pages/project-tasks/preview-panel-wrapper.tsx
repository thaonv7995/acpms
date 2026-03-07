import { useDevServer } from '../../hooks/useDevServer';
import { PreviewPanel } from '../../components/preview/PreviewPanel';

/**
 * PreviewPanelWrapper - Connects PreviewPanel with dev server state
 */
interface PreviewPanelWrapperProps {
  taskId: string;
  attemptId: string;
  fallbackPreviewUrl?: string;
  autoStartOnMount?: boolean;
}

export function PreviewPanelWrapper({
  taskId,
  attemptId,
  fallbackPreviewUrl,
  autoStartOnMount = false,
}: PreviewPanelWrapperProps) {
  const {
    status,
    url,
    errorMessage,
    startServer,
    stopServer,
    restartServer,
    startDisabled,
    startDisabledReason,
    externalPreview,
    previewRevision,
  } = useDevServer(taskId, attemptId, fallbackPreviewUrl, autoStartOnMount);

  return (
    <PreviewPanel
      devServerUrl={url}
      status={status}
      errorMessage={errorMessage}
      externalPreview={externalPreview}
      previewRevision={previewRevision}
      onStart={startServer}
      onStop={stopServer}
      onRestart={restartServer}
      startDisabled={startDisabled}
      startDisabledReason={startDisabledReason}
    />
  );
}
