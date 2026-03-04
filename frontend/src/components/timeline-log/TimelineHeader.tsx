import {
  CheckCircle2,
  RotateCw,
  WifiOff,
  LoaderCircle,
  CircleDot,
  XCircle,
} from 'lucide-react';
import type { AttemptStreamConnectionState } from '@/hooks/useAttemptStream';
import { cn } from '@/lib/utils';
import type { TimelineTokenUsageInfo } from '@/types/timeline-log';

interface TimelineHeaderProps {
  streamState: AttemptStreamConnectionState;
  attemptStatus?: string;
  tokenUsageInfo?: TimelineTokenUsageInfo | null;
  showStatus?: boolean;
  showTokenUsage?: boolean;
}

/**
 * Minimal conversation-style timeline header.
 */
export function TimelineHeader({
  streamState,
  attemptStatus,
  tokenUsageInfo,
  showStatus = true,
  showTokenUsage = true,
}: TimelineHeaderProps) {
  const normalizedAttemptStatus = attemptStatus?.toLowerCase();
  const renderStatus = () => {
    if (normalizedAttemptStatus === 'running' || normalizedAttemptStatus === 'queued') {
      const label = normalizedAttemptStatus === 'queued' ? 'Queued' : 'Running';
      return (
        <>
          <LoaderCircle className="w-3.5 h-3.5 text-primary animate-spin" />
          <span className="text-sm text-primary font-medium">{label}</span>
        </>
      );
    }

    if (normalizedAttemptStatus === 'success' || normalizedAttemptStatus === 'completed') {
      return (
        <>
          <CheckCircle2 className="w-3.5 h-3.5 text-success" />
          <span className="text-sm text-success font-medium">Completed</span>
        </>
      );
    }

    if (normalizedAttemptStatus === 'failed') {
      return (
        <>
          <XCircle className="w-3.5 h-3.5 text-destructive" />
          <span className="text-sm text-destructive font-medium">Failed</span>
        </>
      );
    }

    if (streamState === 'live') {
      return (
        <>
          <div className="relative">
            <div className="w-2 h-2 bg-success rounded-full animate-pulse" />
            <div className="absolute inset-0 w-2 h-2 bg-success rounded-full animate-ping opacity-75" />
          </div>
          <span className="text-sm text-success font-medium">Live</span>
        </>
      );
    }
    if (streamState === 'connecting') {
      return (
        <>
          <LoaderCircle className="w-3.5 h-3.5 text-warning animate-spin" />
          <span className="text-sm text-warning font-medium">Connecting</span>
        </>
      );
    }
    if (streamState === 'reconnecting') {
      return (
        <>
          <RotateCw className="w-3.5 h-3.5 text-warning animate-spin" />
          <span className="text-sm text-warning font-medium">Reconnecting</span>
        </>
      );
    }
    if (streamState === 'stale') {
      return (
        <>
          <CircleDot className="w-3.5 h-3.5 text-orange-400" />
          <span className="text-sm text-orange-400 font-medium">Stale</span>
        </>
      );
    }
    if (streamState === 'offline') {
      return (
        <>
          <WifiOff className="w-3.5 h-3.5 text-muted-foreground" />
          <span className="text-sm text-muted-foreground font-medium">Offline</span>
        </>
      );
    }
    return (
      <>
        <div className="w-2 h-2 bg-muted-foreground/40 rounded-full" />
        <span className="text-sm text-muted-foreground">Idle</span>
      </>
    );
  };

  return (
    <div className="flex items-center justify-between gap-3 px-3 py-2 border-b border-dashed border-border bg-background">
      <div className="flex items-center gap-2 min-w-0">
        {showStatus ? renderStatus() : null}
      </div>

      <div className="flex items-center">
        {showTokenUsage && tokenUsageInfo && (
          <ContextUsageGauge tokenUsageInfo={tokenUsageInfo} />
        )}
      </div>
    </div>
  );
}

function formatTokenCount(value: number): string {
  if (value >= 1_000_000) {
    const millions = value / 1_000_000;
    return millions % 1 === 0 ? `${millions}M` : `${millions.toFixed(1)}M`;
  }
  if (value >= 1_000) {
    return `${Math.round(value / 1_000)}K`;
  }
  return String(value);
}

function ContextUsageGauge({ tokenUsageInfo }: { tokenUsageInfo: TimelineTokenUsageInfo }) {
  const hasContextWindow =
    typeof tokenUsageInfo.modelContextWindow === 'number' &&
    tokenUsageInfo.modelContextWindow > 0;
  const percentage = hasContextWindow
    ? Math.min(100, (tokenUsageInfo.totalTokens / tokenUsageInfo.modelContextWindow!) * 100)
    : 0;
  const progress = Math.min(1, Math.max(0, percentage / 100));

  let colorClass = 'text-emerald-500';
  if (hasContextWindow) {
    if (percentage >= 90) colorClass = 'text-destructive';
    else if (percentage >= 75) colorClass = 'text-orange-500';
    else if (percentage >= 50) colorClass = 'text-amber-500';
  } else {
    colorClass = 'text-sky-500';
  }

  const radius = 8;
  const strokeWidth = 2;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference * (1 - progress);

  const tooltip = hasContextWindow
    ? `Context usage ${Math.round(percentage)}% (${formatTokenCount(tokenUsageInfo.totalTokens)} / ${formatTokenCount(tokenUsageInfo.modelContextWindow!)} tokens)\nInput: ${formatTokenCount(tokenUsageInfo.inputTokens)}\nOutput: ${formatTokenCount(tokenUsageInfo.outputTokens)}`
    : `Token usage\nTotal: ${formatTokenCount(tokenUsageInfo.totalTokens)}\nInput: ${formatTokenCount(tokenUsageInfo.inputTokens)}\nOutput: ${formatTokenCount(tokenUsageInfo.outputTokens)}`;

  return (
    <div
      className="inline-flex items-center justify-center h-7 w-7 rounded-sm border border-border/60 bg-background hover:bg-muted/40 transition-colors cursor-help"
      title={tooltip}
      aria-label={
        hasContextWindow
          ? `Context usage ${Math.round(percentage)} percent`
          : `Token usage ${formatTokenCount(tokenUsageInfo.totalTokens)} total tokens`
      }
    >
      <svg
        viewBox="0 0 20 20"
        className="w-4 h-4 -rotate-90"
        aria-hidden="true"
      >
        <circle
          cx="10"
          cy="10"
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={strokeWidth}
          className="text-border"
        />
        <circle
          cx="10"
          cy="10"
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={strokeWidth}
          strokeLinecap="round"
          strokeDasharray={`${circumference} ${circumference}`}
          strokeDashoffset={dashOffset}
          className={cn(colorClass, 'transition-all duration-500 ease-out')}
        />
      </svg>
    </div>
  );
}
