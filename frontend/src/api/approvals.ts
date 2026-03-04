// API client for tool approval workflow (SDK mode)
import { apiGet, apiPost, API_PREFIX } from './client';

export interface ToolApproval {
  id: string;
  attempt_id: string;
  execution_process_id?: string | null;
  tool_use_id: string;
  tool_name: string;
  tool_input: any;
  status: 'pending' | 'approved' | 'denied' | 'timed_out';
  created_at: string;
}

export interface ApprovalDecisionRequest {
  decision: 'approve' | 'deny';
  reason?: string;
}

export interface ApprovalResponse {
  success: boolean;
  message?: string;
}

/**
 * Get pending approvals for an execution process
 * GET /api/v1/execution-processes/{processId}/approvals/pending
 */
export async function getPendingApprovalsForProcess(processId: string): Promise<ToolApproval[]> {
  return apiGet<ToolApproval[]>(`${API_PREFIX}/execution-processes/${processId}/approvals/pending`);
}

/**
 * Respond to a tool approval request
 * POST /api/v1/approvals/{approvalRef}/respond
 * `approvalRef` accepts approval UUID (preferred) or legacy tool_use_id.
 */
export async function respondToApproval(
  approvalRef: string,
  decision: 'approve' | 'deny',
  reason?: string
): Promise<ApprovalResponse> {
  return apiPost<ApprovalResponse>(`${API_PREFIX}/approvals/${approvalRef}/respond`, {
    decision,
    reason,
  });
}
