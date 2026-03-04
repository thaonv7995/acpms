import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ApprovalModal } from '@/components/modals/ApprovalModal';
import { respondToApproval } from '@/api/approvals';

vi.mock('@/api/approvals', () => ({
  respondToApproval: vi.fn(),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { wrapper, queryClient };
}

const baseApproval = {
  id: 'approval-1',
  attempt_id: 'attempt-1',
  execution_process_id: 'process-1',
  tool_use_id: 'tool-use-1',
  tool_name: 'Bash',
  tool_input: { command: 'ls -la' },
  status: 'pending' as const,
  created_at: '2026-02-27T10:50:00.000Z',
};

describe('ApprovalModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('submits approve decision using tool_use_id ref', async () => {
    vi.mocked(respondToApproval).mockResolvedValue({ success: true } as any);
    const onResponded = vi.fn();
    const onClose = vi.fn();
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');

    render(
      <ApprovalModal
        approval={baseApproval}
        onClose={onClose}
        onResponded={onResponded}
      />,
      { wrapper }
    );

    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));

    await waitFor(() => {
      expect(respondToApproval).toHaveBeenCalledWith('tool-use-1', 'approve', undefined);
      expect(onResponded).toHaveBeenCalledWith('tool-use-1');
      expect(onClose).toHaveBeenCalled();
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ['pending-approvals', 'process-1'],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ['execution-process-logs', 'process-1'],
    });
  });

  it('submits deny decision with optional reason', async () => {
    vi.mocked(respondToApproval).mockResolvedValue({ success: true } as any);
    const onResponded = vi.fn();
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');

    render(
      <ApprovalModal
        approval={baseApproval}
        onClose={vi.fn()}
        onResponded={onResponded}
      />,
      { wrapper }
    );

    fireEvent.click(screen.getByRole('button', { name: 'Deny' }));

    const reasonInput = screen.getByPlaceholderText('Why are you denying this action?');
    fireEvent.change(reasonInput, { target: { value: 'command is unsafe' } });
    fireEvent.click(screen.getByRole('button', { name: 'Confirm Deny' }));

    await waitFor(() => {
      expect(respondToApproval).toHaveBeenCalledWith(
        'tool-use-1',
        'deny',
        'command is unsafe'
      );
      expect(onResponded).toHaveBeenCalledWith('tool-use-1');
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ['pending-approvals', 'process-1'],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ['execution-process-logs', 'process-1'],
    });
  });

  it('falls back to broad approvals invalidation when process scope is missing', async () => {
    vi.mocked(respondToApproval).mockResolvedValue({ success: true } as any);
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries');
    const approvalWithoutProcess = {
      ...baseApproval,
      execution_process_id: null,
    };

    render(
      <ApprovalModal
        approval={approvalWithoutProcess}
        onClose={vi.fn()}
      />,
      { wrapper }
    );

    fireEvent.click(screen.getByRole('button', { name: 'Approve' }));

    await waitFor(() => {
      expect(respondToApproval).toHaveBeenCalledWith('tool-use-1', 'approve', undefined);
    });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['pending-approvals'] });
  });
});
