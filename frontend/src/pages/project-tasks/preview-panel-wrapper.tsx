import { useDevServer } from '../../hooks/useDevServer';
import { PreviewPanel } from '../../components/preview/PreviewPanel';

/**
 * PreviewPanelWrapper - Connects PreviewPanel with dev server state
 */
interface PreviewPanelWrapperProps {
  taskId: string;
  attemptId: string;
}

export function PreviewPanelWrapper({
  taskId,
  attemptId,
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
  } = useDevServer(taskId, attemptId);

  return (
    <PreviewPanel
      devServerUrl={url}
      status={status}
      errorMessage={errorMessage}
      onStart={startServer}
      onStop={stopServer}
      onRestart={restartServer}
      startDisabled={startDisabled}
      startDisabledReason={startDisabledReason}
    />
  );
}
