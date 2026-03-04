import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsPage } from '../../../pages/SettingsPage';
import { useSettings } from '../../../hooks/useSettings';
import { useToast } from '../../../hooks/useToast';
import { useAgentAuthSessionStream } from '../../../hooks/useAgentAuthSessionStream';
import {
  getAgentProvidersStatus,
  initiateAgentAuth,
  type AgentProvidersStatusResponse,
} from '../../../api/settings';

vi.mock('../../../components/layout/AppShell', () => ({
  AppShell: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
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
    provider: 'openai-codex',
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

function makeProvidersStatus(
  entries: Array<{ provider: string; available: boolean; reason: string; message: string }>
): AgentProvidersStatusResponse {
  return {
    default_provider: 'openai-codex',
    providers: entries.map((entry) => ({
      provider: entry.provider,
      installed: true,
      auth_state: entry.available ? 'authenticated' : 'unauthenticated',
      available: entry.available,
      reason: entry.reason as any,
      message: entry.message,
      checked_at: '2026-02-27T12:00:00.000Z',
    })),
  };
}

describe('SettingsPage agent auth UI', () => {
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
  });

  it('renders provider rows with Sign in/Re-auth mapped from availability', async () => {
    vi.mocked(getAgentProvidersStatus).mockResolvedValue(
      makeProvidersStatus([
        {
          provider: 'claude-code',
          available: false,
          reason: 'not_authenticated',
          message: 'Not authenticated',
        },
        {
          provider: 'openai-codex',
          available: true,
          reason: 'ok',
          message: 'Codex CLI is available',
        },
        {
          provider: 'gemini-cli',
          available: false,
          reason: 'cli_missing',
          message: 'Gemini CLI missing',
        },
      ])
    );

    render(<SettingsPage />);

    await waitFor(() => {
      expect(getAgentProvidersStatus).toHaveBeenCalled();
    });

    expect(screen.getByRole('button', { name: 'Re-auth' })).toBeTruthy();
    expect(screen.getAllByRole('button', { name: 'Sign in' })).toHaveLength(2);
  });

  it('uses cached provider status within 24h and skips initial API check', async () => {
    const cached = makeProvidersStatus([
      {
        provider: 'claude-code',
        available: false,
        reason: 'not_authenticated',
        message: 'Not authenticated',
      },
      {
        provider: 'openai-codex',
        available: true,
        reason: 'ok',
        message: 'Codex CLI is available',
      },
      {
        provider: 'gemini-cli',
        available: true,
        reason: 'ok',
        message: 'Gemini CLI is available',
      },
    ]);

    window.localStorage.setItem(
      'agent_provider_status_cache_v1',
      JSON.stringify({
        fetched_at_ms: Date.now(),
        data: cached,
      })
    );

    render(<SettingsPage />);

    expect(await screen.findByText('Codex CLI is available')).toBeTruthy();
    expect(getAgentProvidersStatus).not.toHaveBeenCalled();
  });

  it('opens auth dialog with codex action url and one-time code', async () => {
    vi.mocked(getAgentProvidersStatus).mockResolvedValue(
      makeProvidersStatus([
        {
          provider: 'claude-code',
          available: false,
          reason: 'not_authenticated',
          message: 'Not authenticated',
        },
        {
          provider: 'openai-codex',
          available: true,
          reason: 'ok',
          message: 'Codex CLI is available',
        },
        {
          provider: 'gemini-cli',
          available: false,
          reason: 'not_authenticated',
          message: 'Not authenticated',
        },
      ])
    );

    vi.mocked(initiateAgentAuth).mockResolvedValue({
      session_id: 'session-codex-1',
      provider: 'openai-codex',
      flow_type: 'device_flow',
      status: 'waiting_user_action',
      created_at: '2026-02-27T10:00:00.000Z',
      updated_at: '2026-02-27T10:00:00.000Z',
      expires_at: '2026-02-27T10:05:00.000Z',
      process_pid: 1234,
      allowed_loopback_port: null,
      last_seq: 1,
      last_error: null,
      result: null,
      action_url: 'https://github.com/login/device',
      action_code: 'ABCD-1234',
      action_hint: 'Open URL in browser and enter code.',
    });

    render(<SettingsPage />);

    const [reauthButton] = await screen.findAllByRole('button', { name: 'Re-auth' });
    fireEvent.click(reauthButton);

    await waitFor(() => {
      expect(initiateAgentAuth).toHaveBeenCalledWith('openai-codex');
    });

    expect(await screen.findByText('Provider Authentication')).toBeTruthy();
    expect(screen.getByText('ABCD-1234')).toBeTruthy();
    expect(
      (screen.getByRole('link', { name: 'Open Link' }) as HTMLAnchorElement).getAttribute('href')
    ).toBe('https://github.com/login/device');
    expect(
      screen.getByPlaceholderText('4/0AeaY... or http://127.0.0.1:port/?code=...')
    ).toBeTruthy();
  });

  it('shows terminal auth state without input textarea', async () => {
    vi.mocked(getAgentProvidersStatus).mockResolvedValue(
      makeProvidersStatus([
        {
          provider: 'claude-code',
          available: false,
          reason: 'not_authenticated',
          message: 'Not authenticated',
        },
        {
          provider: 'openai-codex',
          available: true,
          reason: 'ok',
          message: 'Codex CLI is available',
        },
        {
          provider: 'gemini-cli',
          available: false,
          reason: 'not_authenticated',
          message: 'Not authenticated',
        },
      ])
    );

    vi.mocked(initiateAgentAuth).mockResolvedValue({
      session_id: 'session-terminal-1',
      provider: 'openai-codex',
      flow_type: 'device_flow',
      status: 'succeeded',
      created_at: '2026-02-27T10:00:00.000Z',
      updated_at: '2026-02-27T10:01:00.000Z',
      expires_at: '2026-02-27T10:05:00.000Z',
      process_pid: 1234,
      allowed_loopback_port: null,
      last_seq: 2,
      last_error: null,
      result: 'ok',
      action_url: null,
      action_code: null,
      action_hint: null,
    });

    render(<SettingsPage />);

    const [reauthButton] = await screen.findAllByRole('button', { name: 'Re-auth' });
    fireEvent.click(reauthButton);

    await waitFor(() => {
      expect(initiateAgentAuth).toHaveBeenCalledWith('openai-codex');
    });

    const dialog = await screen.findByRole('dialog');
    expect(
      within(dialog).queryByPlaceholderText('4/0AeaY... or http://127.0.0.1:port/?code=...')
    ).toBeNull();
    const submitButton = within(dialog).getByRole('button', { name: 'Submit Input' });
    expect((submitButton as HTMLButtonElement).disabled).toBe(true);
  });

  it('opens claude sign-in mode with loopback hint and no one-time code block', async () => {
    vi.mocked(getAgentProvidersStatus).mockResolvedValue(
      makeProvidersStatus([
        {
          provider: 'claude-code',
          available: false,
          reason: 'auth_expired',
          message: 'Credentials expired',
        },
        {
          provider: 'openai-codex',
          available: true,
          reason: 'ok',
          message: 'Codex CLI is available',
        },
        {
          provider: 'gemini-cli',
          available: true,
          reason: 'ok',
          message: 'Gemini CLI is available',
        },
      ])
    );

    vi.mocked(initiateAgentAuth).mockResolvedValue({
      session_id: 'session-claude-1',
      provider: 'claude-code',
      flow_type: 'loopback_proxy',
      status: 'waiting_user_action',
      created_at: '2026-02-27T10:00:00.000Z',
      updated_at: '2026-02-27T10:00:00.000Z',
      expires_at: '2026-02-27T10:05:00.000Z',
      process_pid: 1111,
      allowed_loopback_port: 53124,
      last_seq: 1,
      last_error: null,
      result: null,
      action_url: 'http://127.0.0.1:53124/callback?code=abc',
      action_code: null,
      action_hint:
        'Complete auth in browser. If redirected to localhost and it fails, paste that localhost URL below.',
    });

    render(<SettingsPage />);

    const signInButton = await screen.findByRole('button', { name: 'Sign in' });
    fireEvent.click(signInButton);

    await waitFor(() => {
      expect(initiateAgentAuth).toHaveBeenCalledWith('claude-code');
    });

    const dialog = await screen.findByRole('dialog');
    expect(
      within(dialog).getByText(
        'Complete auth in browser. If redirected to localhost and it fails, paste that localhost URL below.'
      )
    ).toBeTruthy();
    expect(within(dialog).queryByText('One-time code')).toBeNull();
    expect(within(dialog).getByRole('link', { name: 'Open Link' })).toBeTruthy();
  });

  it('opens gemini sign-in mode with OOB code render', async () => {
    vi.mocked(getAgentProvidersStatus).mockResolvedValue(
      makeProvidersStatus([
        {
          provider: 'claude-code',
          available: true,
          reason: 'ok',
          message: 'Claude CLI is available',
        },
        {
          provider: 'openai-codex',
          available: true,
          reason: 'ok',
          message: 'Codex CLI is available',
        },
        {
          provider: 'gemini-cli',
          available: false,
          reason: 'not_authenticated',
          message: 'Not authenticated',
        },
      ])
    );

    vi.mocked(initiateAgentAuth).mockResolvedValue({
      session_id: 'session-gemini-1',
      provider: 'gemini-cli',
      flow_type: 'oob_code',
      status: 'waiting_user_action',
      created_at: '2026-02-27T10:00:00.000Z',
      updated_at: '2026-02-27T10:00:00.000Z',
      expires_at: '2026-02-27T10:05:00.000Z',
      process_pid: 2222,
      allowed_loopback_port: null,
      last_seq: 1,
      last_error: null,
      result: null,
      action_url: 'https://accounts.google.com/o/oauth2/v2/auth',
      action_code: '4/0AbCdEf123',
      action_hint: 'Open the Google auth URL and submit the code/callback shown by the provider.',
    });

    render(<SettingsPage />);

    const signInButton = await screen.findByRole('button', { name: 'Sign in' });
    fireEvent.click(signInButton);

    await waitFor(() => {
      expect(initiateAgentAuth).toHaveBeenCalledWith('gemini-cli');
    });

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('4/0AbCdEf123')).toBeTruthy();
    expect(
      (
        within(dialog).getByRole('link', { name: 'Open Link' }) as HTMLAnchorElement
      ).getAttribute('href')
    ).toBe('https://accounts.google.com/o/oauth2/v2/auth');
  });
});
