import { Loader2, AlertCircle, CheckCircle2 } from 'lucide-react';

export type DevServerStatus = 'idle' | 'starting' | 'running' | 'stopping' | 'error';

interface DevServerControlsProps {
  status: DevServerStatus;
  url?: string;
  errorMessage?: string;
  startDisabled?: boolean;
  startDisabledReason?: string;
  externalPreview?: boolean;
  className?: string;
}

/**
 * DevServerControls - UI for starting/stopping/restarting dev server
 *
 * @example
 * <DevServerControls
 *   status="running"
 *   url="http://localhost:3000"
 *   onStart={handleStart}
 *   onStop={handleStop}
 *   onRestart={handleRestart}
 * />
 */
export function DevServerControls({
  status,
  url,
  errorMessage,
  startDisabled = false,
  startDisabledReason,
  externalPreview = false,
  className = '',
}: DevServerControlsProps) {
  const isRunning = status === 'running';
  const hasError = status === 'error';

  const getStatusIcon = () => {
    switch (status) {
      case 'starting':
      case 'stopping':
        return <Loader2 className="w-4 h-4 animate-spin" />;
      case 'running':
        return <CheckCircle2 className="w-4 h-4 text-green-600" />;
      case 'error':
        return <AlertCircle className="w-4 h-4 text-red-600" />;
      default:
        return null;
    }
  };

  const getStatusText = () => {
    if (externalPreview) {
      return 'Live preview is available';
    }
    switch (status) {
      case 'starting':
        return 'Starting preview runtime...';
      case 'running':
        return 'Preview is running';
      case 'stopping':
        return 'Stopping preview...';
      case 'error':
        return errorMessage || 'Failed to start dev server';
      default:
        return startDisabledReason || 'Preview is not running';
    }
  };

  const getStatusDetail = () => {
    if (externalPreview) {
      return 'Managed by the agent runtime';
    }
    if (isRunning && url) {
      return 'Managed preview runtime';
    }
    if (startDisabled && startDisabledReason) {
      return startDisabledReason;
    }
    return null;
  };

  const statusDetail = getStatusDetail();

  return (
    <div className={`flex flex-col gap-3 p-4 bg-background border-b border-border ${className}`}>
      {/* Status Bar */}
      <div className="flex items-start gap-3">
        <div className="pt-0.5">{getStatusIcon()}</div>
        <div className="min-w-0">
          <div className="text-sm font-medium text-foreground">{getStatusText()}</div>
          {statusDetail && (
            <div className="text-xs text-muted-foreground">{statusDetail}</div>
          )}
        </div>
      </div>

      {/* Error Message */}
      {hasError && errorMessage && (
        <div className="px-3 py-2 text-sm bg-destructive/10 text-destructive rounded border border-destructive/20">
          {errorMessage}
        </div>
      )}
    </div>
  );
}
