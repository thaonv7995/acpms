import { Play, Square, RotateCw, Loader2, AlertCircle, CheckCircle2 } from 'lucide-react';
import { Button } from '@/components/ui/button';

export type DevServerStatus = 'idle' | 'starting' | 'running' | 'stopping' | 'error';

interface DevServerControlsProps {
  status: DevServerStatus;
  url?: string;
  errorMessage?: string;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  startDisabled?: boolean;
  startDisabledReason?: string;
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
  onStart,
  onStop,
  onRestart,
  startDisabled = false,
  startDisabledReason,
  className = '',
}: DevServerControlsProps) {
  const isStarting = status === 'starting';
  const isRunning = status === 'running';
  const isStopping = status === 'stopping';
  const hasError = status === 'error';
  const startButtonLabel = hasError ? 'Retry' : 'Start';

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
    switch (status) {
      case 'starting':
        return 'Starting dev server...';
      case 'running':
        return url ? `Server running on ${url}` : 'Server running';
      case 'stopping':
        return 'Stopping dev server...';
      case 'error':
        return errorMessage || 'Failed to start dev server';
      default:
        return startDisabledReason || 'Dev server not running';
    }
  };

  return (
    <div className={`flex flex-col gap-3 p-4 bg-background border-b border-border ${className}`}>
      {/* Status Bar */}
      <div className="flex items-center gap-2">
        {getStatusIcon()}
        <span className="text-sm text-foreground">
          {getStatusText()}
        </span>
      </div>

      {/* URL Display (when running) */}
      {isRunning && url && (
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={url}
            readOnly
            className="flex-1 px-3 py-1.5 text-xs font-mono bg-background border border-border rounded focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>
      )}

      {/* Error Message */}
      {hasError && errorMessage && (
        <div className="px-3 py-2 text-sm bg-destructive/10 text-destructive rounded border border-destructive/20">
          {errorMessage}
        </div>
      )}

      {/* Control Buttons */}
      <div className="flex items-center gap-2">
        <Button
          onClick={onStart}
          disabled={isStarting || isRunning || isStopping || startDisabled}
          size="sm"
          variant="default"
          className="gap-2"
          title={startDisabledReason}
        >
          {isStarting ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <Play className="w-4 h-4" />
          )}
          {startButtonLabel}
        </Button>

        <Button
          onClick={onStop}
          disabled={!isRunning || isStopping}
          size="sm"
          variant="outline"
          className="gap-2"
        >
          {isStopping ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <Square className="w-4 h-4" />
          )}
          Stop
        </Button>

        <Button
          onClick={onRestart}
          disabled={!isRunning || isStarting || isStopping}
          size="sm"
          variant="outline"
          className="gap-2"
        >
          <RotateCw className="w-4 h-4" />
          Restart
        </Button>
      </div>
    </div>
  );
}
