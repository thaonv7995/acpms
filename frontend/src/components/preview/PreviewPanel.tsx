import { useState, useRef, useEffect } from 'react';
import { RefreshCw, Maximize2, Minimize2, AlertCircle, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { DevServerControls, DevServerStatus } from './DevServerControls';

interface PreviewPanelProps {
  devServerUrl?: string;
  status: DevServerStatus;
  errorMessage?: string;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  startDisabled?: boolean;
  startDisabledReason?: string;
  className?: string;
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
  onStart,
  onStop,
  onRestart,
  startDisabled = false,
  startDisabledReason,
  className = '',
}: PreviewPanelProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [iframeKey, setIframeKey] = useState(0);
  const [isIframeLoading, setIsIframeLoading] = useState(false);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const isRunning = status === 'running';
  const hasUrl = Boolean(devServerUrl);
  const canShowPreview = isRunning && hasUrl;

  // Reload iframe when URL changes
  useEffect(() => {
    if (devServerUrl) {
      setIframeKey((prev) => prev + 1);
      setIsIframeLoading(true);
    }
  }, [devServerUrl]);

  const handleRefresh = () => {
    setIframeKey((prev) => prev + 1);
    setIsIframeLoading(true);
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
      {/* Dev Server Controls */}
      <DevServerControls
        status={status}
        url={devServerUrl}
        errorMessage={errorMessage}
        onStart={onStart}
        onStop={onStop}
        onRestart={onRestart}
        startDisabled={startDisabled}
        startDisabledReason={startDisabledReason}
      />

      {/* Preview Toolbar (only when running) */}
      {canShowPreview && (
        <div className="flex items-center justify-between px-4 py-2 border-b border-border bg-muted/50">
          <div className="text-xs text-muted-foreground font-mono truncate">
            {devServerUrl}
          </div>
          <div className="flex items-center gap-1">
            <Button
              onClick={handleRefresh}
              size="sm"
              variant="ghost"
              className="gap-2"
              disabled={isIframeLoading}
            >
              <RefreshCw className={`w-4 h-4 ${isIframeLoading ? 'animate-spin' : ''}`} />
              Reload
            </Button>
            <Button
              onClick={handleFullscreen}
              size="sm"
              variant="ghost"
              className="gap-2"
            >
              {isFullscreen ? (
                <>
                  <Minimize2 className="w-4 h-4" />
                  Exit Fullscreen
                </>
              ) : (
                <>
                  <Maximize2 className="w-4 h-4" />
                  Fullscreen
                </>
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
              <p className="text-sm text-muted-foreground mb-4">
                {!isRunning
                  ? 'Start the dev server to see a live preview of your changes.'
                  : 'Waiting for dev server URL...'}
              </p>
              {!isRunning && (
                <Button
                  onClick={onStart}
                  size="sm"
                  className="gap-2"
                  disabled={startDisabled}
                  title={startDisabledReason}
                >
                  <Loader2 className="w-4 h-4" />
                  Start Dev Server
                </Button>
              )}
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
              src={devServerUrl}
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
