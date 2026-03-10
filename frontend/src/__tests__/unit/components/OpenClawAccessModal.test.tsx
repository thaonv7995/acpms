import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { OpenClawAccessModal } from '../../../components/modals/OpenClawAccessModal';
import { useOpenClawAccess } from '../../../hooks/useOpenClawAccess';

vi.mock('../../../hooks/useOpenClawAccess', () => ({
    useOpenClawAccess: vi.fn(),
}));

describe('OpenClawAccessModal', () => {
    const showToast = vi.fn();
    let baseHookReturn: ReturnType<typeof useOpenClawAccess>;

    beforeEach(() => {
        vi.clearAllMocks();
        vi.stubGlobal('navigator', {
            clipboard: {
                writeText: vi.fn().mockResolvedValue(undefined),
            },
        });

        baseHookReturn = {
            clients: [
                {
                    client_id: 'oc_client_prod',
                    display_name: 'OpenClaw Production',
                    status: 'active',
                    kind: 'enrolled',
                    enrolled_at: '2026-03-10T10:00:00Z',
                    expires_at: null,
                    last_seen_at: new Date(Date.now() - 5 * 60 * 1000).toISOString(),
                    last_seen_ip: '203.0.113.10',
                    last_seen_user_agent: 'OpenClaw/1.0.0',
                    key_fingerprints: ['ed25519:ab12cd34'],
                },
                {
                    client_id: 'oc_client_disabled',
                    display_name: 'OpenClaw Staging',
                    status: 'disabled',
                    kind: 'enrolled',
                    enrolled_at: '2026-03-09T10:00:00Z',
                    expires_at: null,
                    last_seen_at: null,
                    last_seen_ip: null,
                    last_seen_user_agent: null,
                    key_fingerprints: [],
                },
            ],
            loading: false,
            error: null,
            latestPrompt: {
                bootstrap_token_id: 'token-1',
                expires_at: '2026-03-10T10:15:00Z',
                prompt_text: 'single-use bootstrap prompt',
                token_preview: 'oc_boot_abcd****',
            },
            creatingPrompt: false,
            activeClientMutationId: null,
            refetchClients: vi.fn(),
            clearLatestPrompt: vi.fn(),
            generateBootstrapPrompt: vi.fn().mockResolvedValue({
                bootstrap_token_id: 'token-1',
                expires_at: '2026-03-10T10:15:00Z',
                prompt_text: 'single-use bootstrap prompt',
                token_preview: 'oc_boot_abcd****',
            }),
            deleteClient: vi.fn().mockResolvedValue(undefined),
            disableClient: vi.fn().mockResolvedValue(undefined),
            enableClient: vi.fn().mockResolvedValue(undefined),
            revokeClient: vi.fn().mockResolvedValue(undefined),
        };

        vi.mocked(useOpenClawAccess).mockReturnValue(baseHookReturn);
    });

    it('renders clients and the latest generated prompt', () => {
        render(
            <OpenClawAccessModal isOpen onClose={vi.fn()} showToast={showToast} />
        );

        expect(screen.getByText('OpenClaw Access')).toBeTruthy();
        expect(screen.getByText('Total installations')).toBeTruthy();
        expect(screen.getByText('OpenClaw Production')).toBeTruthy();
        expect(screen.getByText('OpenClaw Staging')).toBeTruthy();
        expect(screen.getByText('Live now')).toBeTruthy();
        expect(screen.getByText('Never seen')).toBeTruthy();
        expect(screen.getByDisplayValue('single-use bootstrap prompt')).toBeTruthy();
        expect(screen.queryByText('Step 2')).toBeNull();
        expect(screen.getByRole('button', { name: 'Copy Prompt' })).toBeTruthy();
    });

    it('filters installations by search query and status', () => {
        render(
            <OpenClawAccessModal isOpen onClose={vi.fn()} showToast={showToast} />
        );

        fireEvent.change(screen.getByLabelText('Search installations'), {
            target: { value: 'staging' },
        });

        expect(screen.queryByText('OpenClaw Production')).toBeNull();
        expect(screen.getByText('OpenClaw Staging')).toBeTruthy();

        fireEvent.change(screen.getByLabelText('Status filter'), {
            target: { value: 'active' },
        });

        expect(screen.getByText('No installations match the current filters.')).toBeTruthy();
    });

    it('renders waiting connection entries without enrolled-client actions', () => {
        vi.mocked(useOpenClawAccess).mockReturnValue({
            ...baseHookReturn,
            clients: [
                {
                    client_id: 'pending:token-1',
                    display_name: 'OpenClaw QA',
                    status: 'waiting_connection',
                    kind: 'pending',
                    enrolled_at: '2026-03-10T10:05:00Z',
                    expires_at: '2026-03-10T10:20:00Z',
                    last_seen_at: null,
                    last_seen_ip: null,
                    last_seen_user_agent: null,
                    key_fingerprints: [],
                },
            ],
            latestPrompt: null,
        });

        render(
            <OpenClawAccessModal isOpen onClose={vi.fn()} showToast={showToast} />
        );

        expect(screen.getByText('OpenClaw QA')).toBeTruthy();
        expect(screen.getByText('waiting connection')).toBeTruthy();
        expect(screen.getAllByText('Awaiting first enrollment').length).toBeGreaterThan(0);
        expect(screen.queryByRole('button', { name: 'Disable Access' })).toBeNull();
        expect(screen.getByRole('button', { name: 'Delete Installation' })).toBeTruthy();
    });

    it('keeps the add installation panel collapsed until the user opens it', () => {
        vi.mocked(useOpenClawAccess).mockReturnValue({
            ...baseHookReturn,
            latestPrompt: null,
        });

        render(
            <OpenClawAccessModal isOpen onClose={vi.fn()} showToast={showToast} />
        );

        expect(screen.getByRole('button', { name: 'Add another installation' })).toBeTruthy();
        expect(screen.queryByText('Describe the new installation')).toBeNull();

        fireEvent.click(screen.getByRole('button', { name: 'Add another installation' }));

        expect(screen.getByText('Describe the new installation')).toBeTruthy();
        expect(screen.queryByText('Step 1')).toBeNull();
        expect(screen.queryByLabelText('Display name')).toBeNull();
        expect(screen.getByRole('button', { name: 'Hide add panel' })).toBeTruthy();
        expect(screen.getByTestId('openclaw-content-grid').className).toContain(
            'lg:grid-cols-3'
        );
        expect(screen.getByTestId('openclaw-add-panel').className).toContain(
            'lg:col-span-2'
        );
    });

    it('generates a bootstrap prompt from form input', async () => {
        const generateBootstrapPrompt = vi.fn().mockResolvedValue({
            bootstrap_token_id: 'token-2',
            expires_at: '2026-03-10T10:30:00Z',
            prompt_text: 'prompt-2',
            token_preview: 'oc_boot_efgh****',
        });

        vi.mocked(useOpenClawAccess).mockReturnValue({
            ...baseHookReturn,
            generateBootstrapPrompt,
        });

        render(
            <OpenClawAccessModal isOpen onClose={vi.fn()} showToast={showToast} />
        );

        fireEvent.change(screen.getByLabelText('Internal label'), {
            target: { value: 'OpenClaw QA' },
        });

        fireEvent.click(screen.getByRole('button', { name: 'Generate Bootstrap Prompt' }));

        await waitFor(() => {
            expect(generateBootstrapPrompt).toHaveBeenCalledWith({
                label: 'OpenClaw QA',
                expires_in_minutes: 15,
                suggested_display_name: 'OpenClaw QA',
            });
        });
    });

    it('fires disable and enable actions for client rows', async () => {
        const disableClient = vi.fn().mockResolvedValue(undefined);
        const enableClient = vi.fn().mockResolvedValue(undefined);

        vi.mocked(useOpenClawAccess).mockReturnValue({
            ...baseHookReturn,
            disableClient,
            enableClient,
        });

        render(
            <OpenClawAccessModal isOpen onClose={vi.fn()} showToast={showToast} />
        );

        fireEvent.click(screen.getByRole('button', { name: 'Disable Access' }));
        expect(screen.getByRole('dialog', { name: 'Disable access?' })).toBeTruthy();
        fireEvent.click(screen.getByRole('button', { name: 'Disable Client' }));
        await waitFor(() => {
            expect(disableClient).toHaveBeenCalledWith('oc_client_prod');
        });

        fireEvent.click(screen.getByRole('button', { name: 'Enable Access' }));
        await waitFor(() => {
            expect(enableClient).toHaveBeenCalledWith('oc_client_disabled');
        });
    });

    it('clears the latest prompt when generating another installation prompt', async () => {
        const clearLatestPrompt = vi.fn();

        vi.mocked(useOpenClawAccess).mockReturnValue({
            ...baseHookReturn,
            clearLatestPrompt,
        });

        render(
            <OpenClawAccessModal isOpen onClose={vi.fn()} showToast={showToast} />
        );

        fireEvent.click(screen.getByRole('button', { name: 'Generate Another' }));

        expect(clearLatestPrompt).toHaveBeenCalledTimes(1);
    });

    it('keeps the modal open and returns to the installations list when done is pressed', () => {
        const onClose = vi.fn();
        const clearLatestPrompt = vi.fn();

        vi.mocked(useOpenClawAccess).mockReturnValue({
            ...baseHookReturn,
            clearLatestPrompt,
        });

        render(
            <OpenClawAccessModal isOpen onClose={onClose} showToast={showToast} />
        );

        fireEvent.click(screen.getByRole('button', { name: 'Done' }));

        expect(onClose).not.toHaveBeenCalled();
        expect(clearLatestPrompt).toHaveBeenCalledTimes(1);
        expect(screen.queryByDisplayValue('single-use bootstrap prompt')).toBeNull();
        expect(screen.getByRole('button', { name: 'Add another installation' })).toBeTruthy();
    });
});
