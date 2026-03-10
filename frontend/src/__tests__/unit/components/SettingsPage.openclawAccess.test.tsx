import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsPage } from '../../../pages/SettingsPage';
import { useSettings } from '../../../hooks/useSettings';
import { useToast } from '../../../hooks/useToast';
import { useAgentAuthSessionStream } from '../../../hooks/useAgentAuthSessionStream';
import { getAgentProvidersStatus } from '../../../api/settings';

vi.mock('../../../components/layout/AppShell', () => ({
    AppShell: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('../../../components/modals/OpenClawAccessModal', () => ({
    OpenClawAccessModal: ({
        isOpen,
    }: {
        isOpen: boolean;
        onClose: () => void;
        showToast: (message: string, type?: string) => void;
    }) => (isOpen ? <div>OpenClaw Access Modal</div> : null),
}));

vi.mock('../../../hooks/useSettings', () => ({
    useSettings: vi.fn(),
}));

vi.mock('../../../hooks/useToast', () => ({
    useToast: vi.fn(),
}));

vi.mock('../../../hooks/useAgentAuthSessionStream', () => ({
    useAgentAuthSessionStream: vi.fn(),
}));

vi.mock('../../../api/settings', () => ({
    getAgentProvidersStatus: vi.fn(),
    initiateAgentAuth: vi.fn(),
    getAgentAuthSession: vi.fn(),
    submitAgentAuthCode: vi.fn(),
    cancelAgentAuth: vi.fn(),
}));

const baseSettings = {
    gitlab: {
        url: 'https://gitlab.com',
        token: '',
        autoSync: true,
        configured: false,
    },
    agent: {
        provider: 'claude-code',
    },
    openclaw: {
        gatewayEnabled: true,
    },
    cloudflare: {
        accountId: '',
        token: '',
        zoneId: '',
        baseDomain: '',
        configured: false,
    },
    notifications: {
        email: false,
        slack: false,
        slackWebhookUrl: '',
    },
    worktreesPath: './worktrees',
    preferredAgentLanguage: 'en',
};

describe('SettingsPage OpenClaw access section', () => {
    beforeEach(() => {
        vi.clearAllMocks();
        window.localStorage.clear();

        vi.mocked(useSettings).mockReturnValue({
            settings: baseSettings,
            loading: false,
            saving: false,
            testing: { claude: false, gitlab: false },
            error: null,
            refetch: vi.fn(),
            save: vi.fn().mockResolvedValue(undefined),
            testClaude: vi.fn().mockResolvedValue({ success: true, message: 'ok' }),
            testGitLab: vi.fn().mockResolvedValue({ success: true, message: 'ok' }),
        });

        vi.mocked(useToast).mockReturnValue({
            toasts: [],
            showToast: vi.fn(),
            hideToast: vi.fn(),
            clearToasts: vi.fn(),
        });

        vi.mocked(useAgentAuthSessionStream).mockReturnValue({
            session: null,
            isStreaming: true,
            error: null,
            reconnect: vi.fn(),
        });

        vi.mocked(getAgentProvidersStatus).mockResolvedValue({
            default_provider: 'claude-code',
            providers: [
                {
                    provider: 'claude-code',
                    installed: true,
                    auth_state: 'authenticated',
                    available: true,
                    reason: 'ok',
                    message: 'Claude CLI is available',
                    checked_at: '2026-03-10T10:00:00Z',
                },
            ],
        });
    });

    it('opens the OpenClaw access modal from SettingsPage', async () => {
        render(<SettingsPage />);

        await waitFor(() => {
            expect(getAgentProvidersStatus).toHaveBeenCalled();
        });

        fireEvent.click(screen.getByRole('button', { name: /Manage OpenClaw/i }));

        expect(screen.getByText('OpenClaw Access Modal')).toBeTruthy();
    });

    it('hides the OpenClaw access section when gateway is disabled', async () => {
        vi.mocked(useSettings).mockReturnValue({
            settings: {
                ...baseSettings,
                openclaw: {
                    gatewayEnabled: false,
                },
            },
            loading: false,
            saving: false,
            testing: { claude: false, gitlab: false },
            error: null,
            refetch: vi.fn(),
            save: vi.fn().mockResolvedValue(undefined),
            testClaude: vi.fn().mockResolvedValue({ success: true, message: 'ok' }),
            testGitLab: vi.fn().mockResolvedValue({ success: true, message: 'ok' }),
        });

        render(<SettingsPage />);

        await waitFor(() => {
            expect(getAgentProvidersStatus).toHaveBeenCalled();
        });

        expect(screen.queryByText('OpenClaw Access')).toBeNull();
        expect(screen.queryByRole('button', { name: /Manage OpenClaw/i })).toBeNull();
    });
});
