import { CheckCircle, AlertTriangle, RefreshCw, XCircle } from 'lucide-react';

interface SyncStatusIndicatorProps {
  status: 'disconnected' | 'connecting' | 'syncing' | 'synced' | 'reloading';
  error: string | null;
  onRetry?: () => void;
}

export function SyncStatusIndicator({ status, error, onRetry }: SyncStatusIndicatorProps) {
  return (
    <div className="flex items-center gap-2">
      {status === 'synced' && (
        <div className="flex items-center gap-1 text-green-600">
          <CheckCircle className="w-4 h-4" />
          <span className="text-sm">Synced</span>
        </div>
      )}

      {status === 'syncing' && (
        <div className="flex items-center gap-1 text-blue-600">
          <RefreshCw className="w-4 h-4 animate-spin" />
          <span className="text-sm">Syncing...</span>
        </div>
      )}

      {status === 'connecting' && (
        <div className="flex items-center gap-1 text-blue-500">
          <RefreshCw className="w-4 h-4 animate-spin" />
          <span className="text-sm">Connecting...</span>
        </div>
      )}

      {status === 'reloading' && (
        <div className="flex items-center gap-1 text-orange-600">
          <AlertTriangle className="w-4 h-4" />
          <span className="text-sm">Reloading (gap detected)</span>
        </div>
      )}

      {status === 'disconnected' && (
        <div className="flex items-center gap-1 text-red-600">
          <XCircle className="w-4 h-4" />
          <span className="text-sm">Disconnected</span>
          {onRetry && (
            <button
              onClick={onRetry}
              className="ml-2 text-xs underline hover:no-underline"
            >
              Retry
            </button>
          )}
        </div>
      )}

      {error && (
        <div className="text-xs text-red-500 max-w-xs truncate" title={error}>
          {error}
        </div>
      )}
    </div>
  );
}
