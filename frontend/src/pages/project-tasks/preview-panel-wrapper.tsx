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
    dismissPreview,
    restartServer,
    startDisabled,
    startDisabledReason,
    externalPreview,
    previewRevision,
    canStopPreview,
    dismissOnly,
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
      onDismiss={dismissPreview}
      onRestart={restartServer}
      onRebuild={restartServer}
      startDisabled={startDisabled}
      startDisabledReason={startDisabledReason}
      canStopPreview={canStopPreview}
      dismissOnly={dismissOnly}
    />
  );
}
