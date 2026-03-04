// Tool Approval Modal for SDK Mode
import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { respondToApproval } from '../../api/approvals';
import type { ToolApproval } from '../../api/approvals';

interface ApprovalModalProps {
  approval: ToolApproval;
  onClose: () => void;
  onResponded?: (toolUseId: string) => void;
}

export function ApprovalModal({ approval, onClose, onResponded }: ApprovalModalProps) {
  const [denyReason, setDenyReason] = useState('');
  const [showDenyInput, setShowDenyInput] = useState(false);
  const queryClient = useQueryClient();

  const respondMutation = useMutation({
    mutationFn: ({ decision, reason }: { decision: 'approve' | 'deny'; reason?: string }) =>
      respondToApproval(approval.tool_use_id, decision, reason),
    onSuccess: () => {
      // Invalidate process-scoped approval/log snapshots for fallback polling paths.
      if (approval.execution_process_id) {
        queryClient.invalidateQueries({
          queryKey: ['pending-approvals', approval.execution_process_id],
        });
        queryClient.invalidateQueries({
          queryKey: ['execution-process-logs', approval.execution_process_id],
        });
      } else {
        queryClient.invalidateQueries({ queryKey: ['pending-approvals'] });
      }

      // Notify parent
      if (onResponded) {
        onResponded(approval.tool_use_id);
      }

      // Close modal
      onClose();
    },
  });

  const handleApprove = () => {
    respondMutation.mutate({ decision: 'approve' });
  };

  const handleDeny = () => {
    if (!showDenyInput) {
      setShowDenyInput(true);
      return;
    }
    respondMutation.mutate({ decision: 'deny', reason: denyReason || undefined });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
        onClick={respondMutation.isPending ? undefined : onClose}
      ></div>
      <div className="relative w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
        <div className="p-6">
          {/* Header */}
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 rounded-lg bg-orange-100 dark:bg-orange-500/20 text-orange-600 dark:text-orange-400">
              <span className="material-symbols-outlined">security</span>
            </div>
            <div className="flex-1">
              <h2 className="text-lg font-bold text-card-foreground">Tool Permission Request</h2>
              <p className="text-xs text-muted-foreground mt-0.5">
                Claude agent is requesting permission to execute a tool
              </p>
            </div>
          </div>

          {/* Tool Info */}
          <div className="space-y-3 mb-6">
            <div>
              <label className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                Tool Name
              </label>
              <p className="mt-1 px-3 py-2 bg-muted/50 rounded-lg font-mono text-sm text-blue-600 dark:text-blue-400">
                {approval.tool_name}
              </p>
            </div>

            <div>
              <label className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                Tool Input
              </label>
              <pre className="mt-1 px-3 py-2 bg-muted/50 rounded-lg overflow-x-auto text-xs max-h-64 overflow-y-auto">
                <code className="text-card-foreground">
                  {JSON.stringify(approval.tool_input, null, 2)}
                </code>
              </pre>
            </div>
          </div>

          {/* Deny Reason Input (conditional) */}
          {showDenyInput && (
            <div className="mb-4">
              <label className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                Reason for Denial (Optional)
              </label>
              <textarea
                value={denyReason}
                onChange={(e) => setDenyReason(e.target.value)}
                placeholder="Why are you denying this action?"
                className="mt-1 w-full px-3 py-2 bg-muted/50 border border-border rounded-lg resize-none focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm"
                rows={3}
                disabled={respondMutation.isPending}
              />
            </div>
          )}

          {/* Actions */}
          <div className="flex justify-end gap-3">
            <button
              onClick={onClose}
              disabled={respondMutation.isPending}
              className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={handleDeny}
              disabled={respondMutation.isPending}
              className="px-5 py-2 bg-red-600 hover:bg-red-700 text-white text-sm font-bold rounded-lg shadow-lg shadow-red-600/20 transition-all disabled:opacity-50"
            >
              {respondMutation.isPending && respondMutation.variables?.decision === 'deny'
                ? 'Denying...'
                : showDenyInput
                ? 'Confirm Deny'
                : 'Deny'}
            </button>
            <button
              onClick={handleApprove}
              disabled={respondMutation.isPending}
              className="px-5 py-2 bg-green-600 hover:bg-green-700 text-white text-sm font-bold rounded-lg shadow-lg shadow-green-600/20 transition-all disabled:opacity-50"
            >
              {respondMutation.isPending && respondMutation.variables?.decision === 'approve'
                ? 'Approving...'
                : 'Approve'}
            </button>
          </div>

          {/* Error Display */}
          {respondMutation.isError && (
            <div className="mt-4 px-4 py-3 bg-red-100 dark:bg-red-500/20 border border-red-300 dark:border-red-500/50 rounded-lg">
              <p className="text-sm text-red-600 dark:text-red-400">
                Failed to respond to approval. Please try again.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
