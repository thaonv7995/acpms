import { Clock, CheckCircle, XCircle } from 'lucide-react';
import { BaseEntry } from './BaseEntry';
import { RawLogText } from './RawLogText';
import { formatShellCommandForDisplay } from '@/lib/commandDisplay';
import { getActionIcon } from '@/utils/icon-mapping';
import type { ActionType } from '@/bindings/ActionType';
import type { ToolStatus } from '@/bindings/ToolStatus';
import { useApproval } from '@/hooks/useApproval';

interface PendingApprovalEntryProps {
  toolName: string;
  actionType: ActionType;
  status: ToolStatus;
  content: string;
  timestamp?: string | null;
}

/**
 * Display tool call pending user approval with action buttons.
 */
export function PendingApprovalEntry({
  toolName,
  actionType,
  status,
  content,
  timestamp,
}: PendingApprovalEntryProps) {
  const Icon = getActionIcon(actionType.action);
  const formattedCommand =
    actionType.action === 'command_run'
      ? formatShellCommandForDisplay(actionType.command)
      : null;

  // Extract approval details if status is pending_approval
  const approvalId = status.status === 'pending_approval' ? status.approval_id : '';
  const timeoutAt = status.status === 'pending_approval' ? status.timeout_at : '';
  const { approve, deny, isApproving, isDenying, error } = useApproval(approvalId);

  const handleApprove = async () => {
    if (!approvalId || isApproving || isDenying) return;
    try {
      await approve();
    } catch {
      // Error is exposed by useApproval
    }
  };

  const handleDeny = async () => {
    if (!approvalId || isApproving || isDenying) return;
    try {
      await deny('Denied from timeline');
    } catch {
      // Error is exposed by useApproval
    }
  };

  return (
    <BaseEntry variant="action" timestamp={timestamp}>
      <div className="flex items-start gap-3">
        <Clock className="w-5 h-5 text-yellow-500 flex-shrink-0 mt-0.5 animate-pulse" />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-2">
            <Icon className="w-4 h-4 text-primary" />
            <div className="font-medium text-sm">Pending Approval: {toolName}</div>
          </div>

          <div className="text-sm text-muted-foreground mb-3">
            {formattedCommand ? (
              <div className="font-mono break-all">
                Command: {formattedCommand}
              </div>
            ) : (
              <RawLogText text={content} />
            )}
          </div>

          {timeoutAt && (
            <div className="text-xs text-muted-foreground mb-3">
              Timeout: {new Date(timeoutAt).toLocaleTimeString()}
            </div>
          )}

          <div className="flex gap-2">
            <button
              onClick={handleApprove}
              disabled={isApproving || isDenying}
              className="inline-flex items-center gap-2 px-3 py-1.5 bg-green-600 hover:bg-green-500 text-white text-sm rounded font-medium transition-colors"
            >
              <CheckCircle className="w-4 h-4" />
              {isApproving ? 'Approving...' : 'Approve'}
            </button>
            <button
              onClick={handleDeny}
              disabled={isApproving || isDenying}
              className="inline-flex items-center gap-2 px-3 py-1.5 bg-red-600 hover:bg-red-500 text-white text-sm rounded font-medium transition-colors"
            >
              <XCircle className="w-4 h-4" />
              {isDenying ? 'Denying...' : 'Deny'}
            </button>
          </div>

          {error && <div className="text-xs mt-2 text-red-400">{error}</div>}
        </div>
      </div>
    </BaseEntry>
  );
}
