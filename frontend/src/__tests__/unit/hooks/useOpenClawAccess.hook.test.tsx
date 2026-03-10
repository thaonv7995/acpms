import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useOpenClawAccess } from '@/hooks/useOpenClawAccess';
import {
    createOpenClawBootstrapToken,
    deleteOpenClawClient,
    disableOpenClawClient,
    enableOpenClawClient,
    listOpenClawClients,
    revokeOpenClawClient,
} from '@/api/openclawAdmin';

vi.mock('@/api/openclawAdmin', () => ({
    listOpenClawClients: vi.fn(),
    createOpenClawBootstrapToken: vi.fn(),
    deleteOpenClawClient: vi.fn(),
    disableOpenClawClient: vi.fn(),
    enableOpenClawClient: vi.fn(),
    revokeOpenClawClient: vi.fn(),
}));

function createWrapper() {
    const queryClient = new QueryClient({
        defaultOptions: {
            queries: {
                retry: false,
            },
        },
    });

    return function Wrapper({ children }: { children: React.ReactNode }) {
        return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
    };
}

describe('useOpenClawAccess', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        vi.mocked(deleteOpenClawClient).mockResolvedValue({ deleted: {} as never });
        vi.mocked(disableOpenClawClient).mockResolvedValue({ client: {} as never });
        vi.mocked(enableOpenClawClient).mockResolvedValue({ client: {} as never });
        vi.mocked(revokeOpenClawClient).mockResolvedValue({ client: {} as never });
    });

    it('surfaces a newly generated bootstrap prompt as a waiting connection immediately', async () => {
        vi.mocked(listOpenClawClients).mockResolvedValue({
            clients: [
                {
                    client_id: 'oc_client_prod',
                    display_name: 'OpenClaw Production',
                    status: 'active',
                    kind: 'enrolled',
                    enrolled_at: '2026-03-10T10:00:00Z',
                    expires_at: null,
                    last_seen_at: '2026-03-10T10:05:00Z',
                    last_seen_ip: '203.0.113.10',
                    last_seen_user_agent: 'OpenClaw/1.0.0',
                    key_fingerprints: ['ed25519:ab12cd34'],
                },
            ],
        });
        vi.mocked(createOpenClawBootstrapToken).mockResolvedValue({
            bootstrap_token_id: 'token-qa',
            expires_at: '2026-03-10T10:30:00Z',
            prompt_text: 'prompt-qa',
            token_preview: 'oc_boot_qa****',
        });

        const { result } = renderHook(() => useOpenClawAccess(true), {
            wrapper: createWrapper(),
        });

        await waitFor(() => {
            expect(result.current.clients).toHaveLength(1);
        });

        await act(async () => {
            await result.current.generateBootstrapPrompt({
                label: 'OpenClaw QA',
                expires_in_minutes: 15,
                suggested_display_name: 'OpenClaw QA',
            });
        });

        await waitFor(() => {
            expect(result.current.clients).toEqual(
                expect.arrayContaining([
                    expect.objectContaining({
                        client_id: 'pending:token-qa',
                        display_name: 'OpenClaw QA',
                        status: 'waiting_connection',
                        kind: 'pending',
                        expires_at: '2026-03-10T10:30:00Z',
                    }),
                ])
            );
        });
    });
});
