import { useDevServer } from '../../hooks/useDevServer';
import { PreviewPanel } from '../../components/preview/PreviewPanel';

/**
 * PreviewPanelWrapper - Connects PreviewPanel with dev server state
 */
interface PreviewPanelWrapperProps {
  taskId: string;
  attemptId: string;
  fallbackPreviewUrl?: string;
}

export function PreviewPanelWrapper({
  taskId,
  attemptId,
  fallbackPreviewUrl,
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
  } = useDevServer(taskId, attemptId, fallbackPreviewUrl);

  return (
    <PreviewPanel
      devServerUrl={url}
      status={status}
      errorMessage={errorMessage}
      externalPreview={externalPreview}
      onStart={startServer}
      onStop={stopServer}
      onRestart={restartServer}
      startDisabled={startDisabled}
      startDisabledReason={startDisabledReason}
    />
  );
}
