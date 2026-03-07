import React, { useState, useEffect } from 'react';
import { AlertTriangle, Clock } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { AutoExpandingTextarea } from '@/components/ui/auto-expanding-textarea';
import { useApproval } from '@/hooks/useApproval';
import { formatShellCommandForDisplay } from '@/lib/commandDisplay';
import { cn } from '@/lib/utils';

export type ActionType =
  | { action: 'file_edit'; path: string }
  | { action: 'command_run'; command: string }
  | { action: 'deploy'; service: string }
  | { action: 'other'; description: string };

export interface ToolStatus {
  status: 'pending_approval';
  approval_id: string;
  timeout_at: string; // ISO 8601 timestamp
}

export interface PendingApprovalEntryProps {
  toolName: string;
  actionType: ActionType;
  status: ToolStatus;
}

function formatTimeRemaining(timeoutAt: string): string {
  const now = new Date();
  const timeout = new Date(timeoutAt);
  const diffMs = timeout.getTime() - now.getTime();

  if (diffMs <= 0) return '0s';

  const totalSeconds = Math.floor(diffMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

function renderActionDetails(actionType: ActionType): React.ReactNode {
  switch (actionType.action) {
    case 'file_edit':
      return (
        <p className="text-sm">
          Edit file: <code className="bg-muted px-1.5 py-0.5 rounded">{actionType.path}</code>
        </p>
      );
    case 'command_run':
      return (
        <div className="text-sm">
          <p className="mb-1">Run command:</p>
          <pre className="bg-muted p-2 rounded text-xs overflow-x-auto">
            {formatShellCommandForDisplay(actionType.command)}
          </pre>
        </div>
      );
    case 'deploy':
      return (
        <p className="text-sm">
          Deploy service: <code className="bg-muted px-1.5 py-0.5 rounded">{actionType.service}</code>
        </p>
      );
    case 'other':
      return <p className="text-sm">{actionType.description}</p>;
    default:
      return null;
  }
}

export function PendingApprovalEntry({
  toolName,
  actionType,
  status,
}: PendingApprovalEntryProps) {
  const [timeRemaining, setTimeRemaining] = useState(
    formatTimeRemaining(status.timeout_at)
  );
  const [showDenyReason, setShowDenyReason] = useState(false);
  const [denyReason, setDenyReason] = useState('');

  const { approve, deny, isApproving, isDenying, error } = useApproval(
    status.approval_id
  );

  // Update timer every second
  useEffect(() => {
    const interval = setInterval(() => {
      const remaining = formatTimeRemaining(status.timeout_at);
      setTimeRemaining(remaining);

      if (remaining === '0s') {
        clearInterval(interval);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [status.timeout_at]);

  const handleApprove = async () => {
    try {
      await approve();
    } catch {
      // Error is handled by useApproval hook
    }
  };

  const handleDeny = async () => {
    if (!showDenyReason) {
      setShowDenyReason(true);
      return;
    }

    try {
      await deny(denyReason.trim() || undefined);
    } catch {
      // Error is handled by useApproval hook
    }
  };

  const handleCancelDeny = () => {
    setShowDenyReason(false);
    setDenyReason('');
  };

  const isLoading = isApproving || isDenying;
  const timeIsLow = timeRemaining.includes('s') && !timeRemaining.includes('m');

  return (
    <div className="border border-warning/50 bg-warning/5 rounded-lg p-4">
      {/* Header */}
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-2">
          <AlertTriangle className="h-5 w-5 text-warning" />
          <h3 className="font-semibold text-warning">Approval Required</h3>
        </div>
        <div
          className={cn(
            'flex items-center gap-1.5 text-sm font-medium',
            timeIsLow ? 'text-destructive' : 'text-muted-foreground'
          )}
        >
          <Clock className="h-4 w-4" />
          <span>{timeRemaining}</span>
        </div>
      </div>

      {/* Tool details */}
      <div className="space-y-2 mb-4">
        <p className="text-sm font-medium">
          Tool: <span className="text-foreground">{toolName}</span>
        </p>
        {renderActionDetails(actionType)}
      </div>

      {/* Error message */}
      {error && (
        <div className="mb-4 p-3 bg-destructive/10 border border-destructive/20 rounded text-sm text-destructive">
          {error}
        </div>
      )}

      {/* Deny reason input */}
      {showDenyReason && (
        <div className="mb-4">
          <AutoExpandingTextarea
            value={denyReason}
            onChange={(e) => setDenyReason(e.target.value)}
            placeholder="Reason for denial (optional)"
            disabled={isLoading}
            minHeight={60}
            maxHeight={120}
          />
        </div>
      )}

      {/* Action buttons */}
      <div className="flex items-center gap-2">
        <Button
          onClick={handleApprove}
          disabled={isLoading || showDenyReason}
          variant="default"
          size="sm"
          className="bg-success hover:bg-success/90"
        >
          {isApproving ? 'Approving...' : 'Approve'}
        </Button>

        <Button
          onClick={handleDeny}
          disabled={isLoading}
          variant="outline"
          size="sm"
          className="border-destructive text-destructive hover:bg-destructive/10"
        >
          {isDenying ? 'Denying...' : showDenyReason ? 'Confirm Deny' : 'Deny'}
        </Button>

        {showDenyReason && (
          <Button
            onClick={handleCancelDeny}
            disabled={isLoading}
            variant="ghost"
            size="sm"
          >
            Cancel
          </Button>
        )}
      </div>
    </div>
  );
}
