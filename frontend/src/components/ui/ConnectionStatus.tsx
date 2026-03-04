import { Wifi, WifiOff, RotateCw, AlertCircle } from 'lucide-react';
import { ConnectionStatus as Status } from '@/types/websocket.types';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';

export interface ConnectionStatusProps {
  status: Status;
  error?: string | null;
  onReconnect?: () => void;
  className?: string;
}

export function ConnectionStatus({
  status,
  error,
  onReconnect,
  className,
}: ConnectionStatusProps) {
  const getStatusConfig = () => {
    switch (status) {
      case 'connected':
        return {
          icon: Wifi,
          text: 'Connected',
          color: 'text-success',
          bgColor: 'bg-success/10',
          showReconnect: false,
        };
      case 'connecting':
        return {
          icon: RotateCw,
          text: 'Connecting...',
          color: 'text-blue-500',
          bgColor: 'bg-blue-500/10',
          showReconnect: false,
          animate: true,
        };
      case 'reconnecting':
        return {
          icon: RotateCw,
          text: 'Reconnecting...',
          color: 'text-yellow-500',
          bgColor: 'bg-yellow-500/10',
          showReconnect: true,
          animate: true,
        };
      case 'disconnected':
        return {
          icon: WifiOff,
          text: 'Disconnected',
          color: 'text-muted-foreground',
          bgColor: 'bg-muted',
          showReconnect: true,
        };
      case 'error':
        return {
          icon: AlertCircle,
          text: error || 'Connection Error',
          color: 'text-destructive',
          bgColor: 'bg-destructive/10',
          showReconnect: true,
        };
      default:
        return {
          icon: WifiOff,
          text: 'Idle',
          color: 'text-muted-foreground',
          bgColor: 'bg-muted',
          showReconnect: false,
        };
    }
  };

  const config = getStatusConfig();
  const Icon = config.icon;

  if (status === 'idle' || status === 'connected') {
    // Don't show for idle or successful connection
    return null;
  }

  return (
    <div
      className={cn(
        'flex items-center gap-2 px-3 py-1.5 rounded-md text-sm',
        config.bgColor,
        className
      )}
    >
      <Icon
        className={cn('h-4 w-4', config.color, config.animate && 'animate-spin')}
      />
      <span className={config.color}>{config.text}</span>

      {config.showReconnect && onReconnect && (
        <Button
          onClick={onReconnect}
          variant="ghost"
          size="sm"
          className="h-6 px-2 ml-auto"
        >
          Reconnect
        </Button>
      )}
    </div>
  );
}
