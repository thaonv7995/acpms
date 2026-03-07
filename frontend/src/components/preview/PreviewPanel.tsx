import { useState, useRef, useEffect } from 'react';
import {
  ExternalLink,
  Hammer,
  Loader2,
  Maximize2,
  Minimize2,
  Pencil,
  Play,
  RefreshCw,
  RotateCw,
  Square,
  CheckCircle2,
  AlertCircle,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { DevServerStatus } from './DevServerControls';

interface PreviewPanelProps {
  devServerUrl?: string;
  status: DevServerStatus;
  errorMessage?: string;
  externalPreview?: boolean;
  previewRevision?: number;
  onStart: () => void;
  onStop: () => void;
  onDismiss: () => void;
  onRestart: () => void;
  onRebuild?: () => void;
  startDisabled?: boolean;
  startDisabledReason?: string;
  startActionTitle?: string;
  startActionLabel?: string;
  canStopPreview?: boolean;
  dismissOnly?: boolean;
  className?: string;
}

function buildIframeSrc(
  devServerUrl?: string,
  externalPreview?: boolean,
  previewRevision?: number
): string | undefined {
  if (!devServerUrl) {
    return undefined;
  }

  if (!externalPreview || !previewRevision) {
    return devServerUrl;
  }

  try {
    const url = new URL(devServerUrl);
    url.searchParams.set('acpms_preview_rev', String(previewRevision));
    return url.toString();
  } catch {
    const separator = devServerUrl.includes('?') ? '&' : '?';
    return `${devServerUrl}${separator}acpms_preview_rev=${previewRevision}`;
  }
}

/**
 * PreviewPanel - Iframe container for dev server preview
 * Shows controls for managing dev server and displays preview in iframe
 *
 * @example
 * <PreviewPanel
 *   devServerUrl="http://localhost:3000"
 *   status="running"
 *   onStart={handleStart}
 *   onStop={handleStop}
 *   onRestart={handleRestart}
 * />
 */
export function PreviewPanel({
  devServerUrl,
  status,
  errorMessage,
  externalPreview = false,
  previewRevision = 0,
  onStart,
  onStop,
  onDismiss,
  onRestart,
  onRebuild,
  startDisabled = false,
  startDisabledReason,
  startActionTitle,
  startActionLabel = 'Start preview',
  canStopPreview = false,
  dismissOnly = false,
  className = '',
}: PreviewPanelProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [iframeKey, setIframeKey] = useState(0);
  const [isIframeLoading, setIsIframeLoading] = useState(false);
  const [manualUrlOverride, setManualUrlOverride] = useState<string | undefined>();
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const effectiveUrl = manualUrlOverride?.trim() || devServerUrl;
  const iframeSrc = buildIframeSrc(effectiveUrl, externalPreview, previewRevision);

  const isRunning = status === 'running';
  const isStarting = status === 'starting';
  const isStopping = status === 'stopping';
  const hasError = status === 'error';
  const hasUrl = Boolean(effectiveUrl);
  const hasManualOverride = Boolean(manualUrlOverride?.trim());
  const canShowPreview = hasUrl && (isRunning || externalPreview || hasManualOverride);
  const canManageRuntime = !externalPreview;
  const effectiveRebuild = onRebuild ?? onRestart;

  const statusIcon = (() => {
    if (hasError) {
      return <AlertCircle className="w-4 h-4 text-destructive shrink-0" />;
    }
    if (isStarting || isStopping) {
      return <Loader2 className="w-4 h-4 animate-spin text-muted-foreground shrink-0" />;
    }
    if (canShowPreview || isRunning) {
      return <CheckCircle2 className="w-4 h-4 text-green-600 shrink-0" />;
    }
    return <Play className="w-4 h-4 text-muted-foreground shrink-0" />;
  })();

  const statusText = (() => {
    if (effectiveUrl) {
      return effectiveUrl;
    }
    if (hasError) {
      return errorMessage || 'Preview failed to start';
    }
    if (isStarting) {
      return 'Starting preview...';
    }
    if (isStopping) {
      return 'Stopping preview...';
    }
    return startDisabledReason || 'Preview is not running';
  })();

  // Reload iframe when the preview source changes. External preview may reuse
  // the same URL across follow-ups, so previewRevision forces a refresh.
  useEffect(() => {
    if (iframeSrc) {
      setIframeKey((prev) => prev + 1);
      setIsIframeLoading(true);
    }
  }, [iframeSrc]);

  const handleRefresh = () => {
    setIframeKey((prev) => prev + 1);
    setIsIframeLoading(true);
  };

  const handleOpen = () => {
    if (!iframeSrc) return;
    window.open(iframeSrc, '_blank', 'noopener,noreferrer');
  };

  const handleEditUrl = () => {
    const currentValue = manualUrlOverride?.trim() || devServerUrl || '';
    const nextValue = window.prompt('Edit preview URL', currentValue);
    if (nextValue === null) {
      return;
    }
    const trimmed = nextValue.trim();
    setManualUrlOverride(trimmed.length > 0 ? trimmed : undefined);
    setIframeKey((prev) => prev + 1);
    setIsIframeLoading(Boolean(trimmed.length > 0 || devServerUrl));
  };

  const handleFullscreen = () => {
    if (!isFullscreen && containerRef.current) {
      containerRef.current.requestFullscreen?.();
      setIsFullscreen(true);
    } else if (isFullscreen && document.fullscreenElement) {
      document.exitFullscreen();
      setIsFullscreen(false);
    }
  };

  const handleIframeLoad = () => {
    setIsIframeLoading(false);
  };

  const handleIframeError = () => {
    setIsIframeLoading(false);
  };

  useEffect(() => {
    const handleFullscreenChange = () => {
      setIsFullscreen(Boolean(document.fullscreenElement));
    };

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    return () => document.removeEventListener('fullscreenchange', handleFullscreenChange);
  }, []);

  return (
    <div
      ref={containerRef}
      className={`flex flex-col h-full bg-background ${className}`}
    >
      {/* Preview Action Bar */}
      {(canShowPreview || canManageRuntime) && (
        <div className="flex items-center justify-between gap-3 px-3 py-1.5 border-b border-border bg-muted/50">
          <div className="min-w-0 flex-1 flex items-center gap-2">
            {statusIcon}
            <div
              className={`text-xs truncate leading-5 ${
                devServerUrl ? 'text-muted-foreground font-mono' : hasError ? 'text-destructive' : 'text-muted-foreground'
              }`}
            >
              {statusText}
            </div>
          </div>
          <div className="flex items-center gap-1 shrink-0">
            {!canShowPreview && canManageRuntime && (
              <Button
                onClick={onStart}
                size="icon"
                variant="outline"
                className="h-8 w-8 text-emerald-400 hover:text-emerald-300 hover:bg-emerald-500/10"
                disabled={isStarting || isStopping || startDisabled}
                title={startActionTitle || startDisabledReason || startActionLabel}
                aria-label={startActionLabel}
              >
                {isStarting ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Play className="w-4 h-4" />
                )}
              </Button>
            )}
            {canShowPreview && (
              <Button
                onClick={handleOpen}
                size="icon"
                variant="ghost"
                className="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
                title="Open preview in a new tab"
                aria-label="Open preview in a new tab"
              >
                <ExternalLink className="w-4 h-4" />
              </Button>
            )}
            <Button
              onClick={handleEditUrl}
              size="icon"
              variant="ghost"
              className="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
              title="Edit preview URL"
              aria-label="Edit preview URL"
            >
              <Pencil className="w-4 h-4" />
            </Button>
            <Button
              onClick={handleRefresh}
              size="icon"
              variant="ghost"
              className="h-8 w-8 text-sky-400 hover:text-sky-300 hover:bg-sky-500/10"
              disabled={!canShowPreview || isIframeLoading}
              title="Reload the current preview view"
              aria-label="Refresh preview"
            >
              <RefreshCw className={`w-4 h-4 ${isIframeLoading ? 'animate-spin' : ''}`} />
            </Button>
            {canManageRuntime && canShowPreview && (
              <>
                <Button
                  onClick={onRestart}
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 text-amber-400 hover:text-amber-300 hover:bg-amber-500/10"
                  disabled={!isRunning || isStarting || isStopping}
                  title="Restart the running preview service"
                  aria-label="Restart preview runtime"
                >
                  <RotateCw className="w-4 h-4" />
                </Button>
                <Button
                  onClick={effectiveRebuild}
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 text-cyan-400 hover:text-cyan-300 hover:bg-cyan-500/10"
                  disabled={!isRunning || isStarting || isStopping}
                  title="Rebuild preview from the latest source"
                  aria-label="Rebuild preview from the latest source"
                >
                  <Hammer className="w-4 h-4" />
                </Button>
              </>
            )}
            {canStopPreview && (
              <Button
                onClick={onStop}
                size="icon"
                variant="ghost"
                className="h-8 w-8 text-rose-400 hover:text-rose-300 hover:bg-rose-500/10"
                disabled={isStopping}
                title="Stop preview"
                aria-label="Stop preview"
              >
                <Square className="w-4 h-4" />
              </Button>
            )}
            {!canStopPreview && dismissOnly && canShowPreview && (
              <Button
                onClick={onDismiss}
                size="icon"
                variant="ghost"
                className="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
                title="Dismiss preview"
                aria-label="Dismiss preview"
              >
                <X className="w-4 h-4" />
              </Button>
            )}
            <Button
              onClick={handleFullscreen}
              size="icon"
              variant="ghost"
              className="h-8 w-8 text-muted-foreground hover:text-foreground hover:bg-muted"
              title={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
              aria-label={isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'}
            >
              {isFullscreen ? (
                <Minimize2 className="w-4 h-4" />
              ) : (
                <Maximize2 className="w-4 h-4" />
              )}
            </Button>
          </div>
        </div>
      )}

      {/* Preview Content Area */}
      <div className="flex-1 min-h-0 relative bg-muted/30">
        {!canShowPreview ? (
          // Empty State
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center px-6 py-8 max-w-md">
              <AlertCircle className="w-12 h-12 text-muted-foreground/50 mx-auto mb-4" />
              <h3 className="text-lg font-semibold text-foreground mb-2">
                No Preview Available
              </h3>
              <p className="text-sm text-muted-foreground">
                {!isRunning
                  ? 'Use the action bar above to start or request a preview for this task.'
                  : 'Waiting for the preview URL...'}
              </p>
            </div>
          </div>
        ) : (
          // Iframe Preview
          <>
            {isIframeLoading && (
              <div className="absolute inset-0 flex items-center justify-center bg-background/80 z-10">
                <div className="text-center">
                  <Loader2 className="w-8 h-8 animate-spin text-primary mx-auto mb-2" />
                  <p className="text-sm text-muted-foreground">
                    Loading preview...
                  </p>
                </div>
              </div>
            )}
            <iframe
              key={iframeKey}
              ref={iframeRef}
              src={iframeSrc}
              onLoad={handleIframeLoad}
              onError={handleIframeError}
              sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-modals"
              allow="geolocation; microphone; camera"
              className="w-full h-full border-0"
              title="Dev Server Preview"
            />
          </>
        )}
      </div>
    </div>
  );
}
