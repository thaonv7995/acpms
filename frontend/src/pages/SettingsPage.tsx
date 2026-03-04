import { useState, useEffect, useRef } from 'react';
import { AppShell } from '../components/layout/AppShell';
import { useSettings } from '../hooks/useSettings';
import { useToast } from '../hooks/useToast';
import { Toast } from '../components/shared/Toast';
import {
    getAgentProvidersStatus,
    initiateAgentAuth,
    getAgentAuthSession,
    submitAgentAuthCode,
    cancelAgentAuth,
    type AgentProviderStatus,
    type AgentProvidersStatusResponse,
    type AgentAuthSession,
} from '../api/settings';
import { ApiError } from '../api/client';
import { useAgentAuthSessionStream } from '../hooks/useAgentAuthSessionStream';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from '../components/ui/dialog';
import { logger } from '@/lib/logger';

type SettingsGuideType = 'source_control' | 'agent_auth' | 'cloudflare' | null;

interface SettingsGuideStep {
    label: string;
    title: string;
    detail: string;
    icon: string;
    hint?: string;
}

interface SettingsGuideContent {
    title: string;
    description: string;
    audienceLabel: string;
    prep: string[];
    steps: SettingsGuideStep[];
    note?: string;
}

const SETTINGS_GUIDES: Record<
    Exclude<SettingsGuideType, null>,
    SettingsGuideContent
> = {
    source_control: {
        title: 'Connect Source Control (GitLab / GitHub)',
        description:
            'Get your URL and Personal Access Token, then paste them into this form.',
        audienceLabel: 'Beginner Friendly',
        prep: [
            'A GitLab or GitHub account',
            'Permission to create a Personal Access Token',
        ],
        steps: [
            {
                label: 'Step 1',
                title: 'Open the token creation page',
                detail:
                    'GitLab: User Settings -> Access Tokens. GitHub: Settings -> Developer settings -> Personal access tokens.',
                icon: 'manage_accounts',
            },
            {
                label: 'Step 2',
                title: 'Create a token with the right scopes',
                detail:
                    'GitLab: api + read_repository + write_repository. GitHub: repo + workflow.',
                icon: 'vpn_key',
            },
            {
                label: 'Step 3',
                title: 'Paste it into this settings page',
                detail:
                    'Fill in GitLab Instance URL + Personal Access Token, then click Save.',
                icon: 'content_paste',
            },
        ],
        note:
            'Do not share tokens in chat/email. Use a dedicated service token when possible.',
    },
    agent_auth: {
        title: 'Authenticate AI Agent',
        description:
            'Choose a provider and authenticate directly from Settings using Sign in/Re-auth.',
        audienceLabel: 'Setup in 2-3 Minutes',
        prep: [
            'Server has the provider CLI installed',
            'A valid Claude/OpenAI/Gemini/Cursor account',
        ],
        steps: [
            {
                label: 'Step 1',
                title: 'Choose your provider',
                detail:
                    'Pick Claude Code, OpenAI Codex, Gemini CLI, or Cursor CLI in Agent CLI Provider.',
                icon: 'hub',
            },
            {
                label: 'Step 2',
                title: 'Start auth from this page',
                detail:
                    'Use Sign in/Re-auth on the provider row. The system starts an auth session and guides required actions.',
                icon: 'terminal',
            },
            {
                label: 'Step 3',
                title: 'Complete provider verification',
                detail:
                    'After verification, provider status becomes Available. Then it can be used as default runtime.',
                icon: 'refresh',
            },
        ],
        note:
            'Auth sessions are initiated from UI but execute on the backend host runtime.',
    },
    cloudflare: {
        title: 'Set Up Cloudflare Preview',
        description:
            'To enable Preview, all 4 fields are required: Account ID, Zone ID, API Token, and Base Domain.',
        audienceLabel: '4 Required Items',
        prep: [
            'A domain already managed in Cloudflare',
            'Permission to create API Tokens',
        ],
        steps: [
            {
                label: 'Step 1',
                title: 'Get Account ID',
                detail:
                    'In Cloudflare Dashboard, find it on the right sidebar or Workers & Pages page.',
                icon: 'badge',
            },
            {
                label: 'Step 2',
                title: 'Get Zone ID',
                detail:
                    'Open your domain -> Overview tab -> Zone ID.',
                icon: 'dns',
            },
            {
                label: 'Step 3',
                title: 'Create API Token',
                detail:
                    'My Profile -> API Tokens -> Create token (Tunnel + DNS edit permissions for target zone).',
                icon: 'api',
            },
            {
                label: 'Step 4',
                title: 'Fill Base Domain',
                detail:
                    'Example: `previews.example.com`. Then click Save.',
                icon: 'public',
            },
        ],
        note:
            'If Preview is still blocked, verify all 4 fields are populated first.',
    },
};

const PROVIDER_LABELS: Record<string, string> = {
    'claude-code': 'Claude Code (Anthropic)',
    'openai-codex': 'Codex CLI (OpenAI)',
    'gemini-cli': 'Gemini CLI (Google)',
    'cursor-cli': 'Cursor CLI',
};
const AGENT_AUTH_SESSION_STORAGE_KEY = 'agent_auth_session_id';
const AGENT_PROVIDER_STATUS_CACHE_KEY = 'agent_provider_status_cache_v1';
const AGENT_PROVIDER_STATUS_CACHE_TTL_MS = 24 * 60 * 60 * 1000;

type AgentProviderStatusCache = {
    fetched_at_ms: number;
    data: AgentProvidersStatusResponse;
};

function readCachedAgentProviderStatus(): AgentProvidersStatusResponse | null {
    if (typeof window === 'undefined') return null;
    try {
        const raw = window.localStorage.getItem(AGENT_PROVIDER_STATUS_CACHE_KEY);
        if (!raw) return null;
        const parsed = JSON.parse(raw) as Partial<AgentProviderStatusCache>;
        if (
            typeof parsed?.fetched_at_ms !== 'number' ||
            !parsed.data ||
            !Array.isArray(parsed.data.providers)
        ) {
            window.localStorage.removeItem(AGENT_PROVIDER_STATUS_CACHE_KEY);
            return null;
        }
        if (Date.now() - parsed.fetched_at_ms > AGENT_PROVIDER_STATUS_CACHE_TTL_MS) {
            window.localStorage.removeItem(AGENT_PROVIDER_STATUS_CACHE_KEY);
            return null;
        }
        return parsed.data;
    } catch {
        window.localStorage.removeItem(AGENT_PROVIDER_STATUS_CACHE_KEY);
        return null;
    }
}

function writeCachedAgentProviderStatus(data: AgentProvidersStatusResponse): void {
    if (typeof window === 'undefined') return;
    const payload: AgentProviderStatusCache = {
        fetched_at_ms: Date.now(),
        data,
    };
    window.localStorage.setItem(AGENT_PROVIDER_STATUS_CACHE_KEY, JSON.stringify(payload));
}

function providerReasonLabel(status: AgentProviderStatus | undefined): string {
    if (!status) return 'No status available yet';
    switch (status.reason) {
        case 'cli_missing':
            return 'CLI not installed';
        case 'not_authenticated':
            return 'Not authenticated';
        case 'auth_expired':
            return 'Credentials expired';
        case 'auth_check_failed':
            return 'Auth check failed';
        default:
            return status.message;
    }
}

function isAuthTerminal(status: AgentAuthSession['status'] | undefined): boolean {
    return (
        status === 'succeeded' ||
        status === 'failed' ||
        status === 'cancelled' ||
        status === 'timed_out'
    );
}

function SettingsGuideButton({
    onClick,
    title,
}: {
    onClick: () => void;
    title: string;
}) {
    return (
        <button
            type="button"
            onClick={onClick}
            title={title}
            className="inline-flex items-center justify-center size-6 rounded-full border border-border text-muted-foreground hover:text-primary hover:border-primary/50 hover:bg-muted transition-colors"
            aria-label={title}
        >
            <span className="material-symbols-outlined text-[16px]">info</span>
        </button>
    );
}

export function SettingsPage() {
    const { settings, loading: settingsLoading, saving, save } = useSettings();
    const { toasts, showToast, hideToast } = useToast();

    // GitLab State
    const [gitlabUrl, setGitlabUrl] = useState('');
    const [gitlabToken, setGitlabToken] = useState('');
    const [showGitlabToken, setShowGitlabToken] = useState(false);
    const [isEditingGitlab, setIsEditingGitlab] = useState(false);

    // Cloudflare State
    // Cloudflare State
    const [cfAccountId, setCfAccountId] = useState('');
    const [cfToken, setCfToken] = useState('');
    const [cfZoneId, setCfZoneId] = useState('');
    const [cfBaseDomain, setCfBaseDomain] = useState('');
    const [showCfToken, setShowCfToken] = useState(false);
    const [isEditingCloudflare, setIsEditingCloudflare] = useState(false);

    // Worktrees Path & Agent Language State
    const [worktreesPath, setWorktreesPath] = useState('');
    const [preferredAgentLanguage, setPreferredAgentLanguage] = useState<'en' | 'vi'>('en');
    const [isEditingWorktrees, setIsEditingWorktrees] = useState(false);

    // Agent CLI State
    const [agentProvider, setAgentProvider] = useState('claude-code');
    const [isEditingAgent, setIsEditingAgent] = useState(false);
    const [activeGuide, setActiveGuide] = useState<SettingsGuideType>(null);

    const [agentProvidersStatus, setAgentProvidersStatus] =
        useState<AgentProvidersStatusResponse | null>(null);
    const [agentLoading, setAgentLoading] = useState(true);
    const [agentAuthLoadingProvider, setAgentAuthLoadingProvider] = useState<string | null>(null);
    const [activeAuthSession, setActiveAuthSession] = useState<AgentAuthSession | null>(null);
    const [showAuthDialog, setShowAuthDialog] = useState(false);
    const [authInput, setAuthInput] = useState('');
    const [authSubmitting, setAuthSubmitting] = useState(false);
    const [deviceCodeCopied, setDeviceCodeCopied] = useState(false);
    const handledTerminalAuthSeqRef = useRef<string | null>(null);
    const openedAuthUrlSessionIdRef = useRef<string | null>(null);
    const authSessionStream = useAgentAuthSessionStream(activeAuthSession?.session_id);

    // Fetch agent status on mount
    useEffect(() => {
        const fetchAgentStatus = async () => {
            const cached = readCachedAgentProviderStatus();
            if (cached) {
                setAgentProvidersStatus(cached);
                setAgentLoading(false);
                return;
            }
            try {
                const response = await getAgentProvidersStatus();
                setAgentProvidersStatus(response);
                writeCachedAgentProviderStatus(response);
            } catch (err) {
                logger.error('Failed to fetch agent status:', err);
                setAgentProvidersStatus(null);
            } finally {
                setAgentLoading(false);
            }
        };
        fetchAgentStatus();
    }, []);

    useEffect(() => {
        const restoreSession = async () => {
            const sessionId = window.localStorage.getItem(AGENT_AUTH_SESSION_STORAGE_KEY);
            if (!sessionId) return;
            try {
                const session = await getAgentAuthSession(sessionId);
                if (!isAuthTerminal(session.status)) {
                    setActiveAuthSession(session);
                    setShowAuthDialog(true);
                } else {
                    window.localStorage.removeItem(AGENT_AUTH_SESSION_STORAGE_KEY);
                }
            } catch {
                window.localStorage.removeItem(AGENT_AUTH_SESSION_STORAGE_KEY);
            }
        };
        void restoreSession();
    }, []);

    // Initialize from settings
    useEffect(() => {
        if (settings) {
            setGitlabUrl(settings.gitlab?.url || 'https://gitlab.com');
            setGitlabToken(settings.gitlab?.token || '');
            setWorktreesPath(settings.worktreesPath || './worktrees');
            setPreferredAgentLanguage((settings.preferredAgentLanguage === 'vi' ? 'vi' : 'en') as 'en' | 'vi');
            setAgentProvider(settings.agent?.provider || 'claude-code');
            setCfAccountId(settings.cloudflare?.accountId || '');
            setCfToken(settings.cloudflare?.token || '');
            setCfZoneId(settings.cloudflare?.zoneId || '');
            setCfBaseDomain(settings.cloudflare?.baseDomain || '');
        }
    }, [settings]);

    useEffect(() => {
        const streamedSession = authSessionStream.session;
        if (!streamedSession) return;
        setActiveAuthSession((current) => {
            if (!current) return streamedSession;
            if (current.session_id !== streamedSession.session_id) return current;
            return streamedSession;
        });
    }, [authSessionStream.session]);

    useEffect(() => {
        if (!activeAuthSession?.session_id) return;
        if (isAuthTerminal(activeAuthSession.status)) return;
        if (authSessionStream.isStreaming) return;

        let stopped = false;
        const sessionId = activeAuthSession.session_id;

        const pollSession = async () => {
            try {
                const latest = await getAgentAuthSession(sessionId);
                if (stopped) return;
                setActiveAuthSession((current) =>
                    current?.session_id === sessionId ? latest : current
                );
            } catch {
                if (!stopped) {
                    setActiveAuthSession((current) =>
                        current?.session_id === sessionId
                            ? {
                                ...current,
                                status: 'failed',
                                last_error:
                                    current.last_error ||
                                    'Failed to refresh auth session state',
                            }
                            : current
                    );
                }
            }
        };

        void pollSession();
        const timer = window.setInterval(() => {
            void pollSession();
        }, 2000);

        return () => {
            stopped = true;
            window.clearInterval(timer);
        };
    }, [activeAuthSession?.session_id, activeAuthSession?.status, authSessionStream.isStreaming]);

    useEffect(() => {
        if (!activeAuthSession?.session_id) {
            handledTerminalAuthSeqRef.current = null;
            return;
        }
        if (!isAuthTerminal(activeAuthSession.status)) {
            handledTerminalAuthSeqRef.current = null;
            return;
        }

        const terminalKey = `${activeAuthSession.session_id}:${activeAuthSession.last_seq}:${activeAuthSession.status}`;
        if (handledTerminalAuthSeqRef.current === terminalKey) {
            return;
        }
        handledTerminalAuthSeqRef.current = terminalKey;

        void handleRefreshAgentStatus();
        if (activeAuthSession.status === 'succeeded') {
            showToast(
                `${PROVIDER_LABELS[activeAuthSession.provider] || activeAuthSession.provider} authenticated successfully`,
                'success'
            );
        }
    }, [activeAuthSession?.session_id, activeAuthSession?.status, activeAuthSession?.last_seq]);

    useEffect(() => {
        if (!activeAuthSession?.session_id) {
            window.localStorage.removeItem(AGENT_AUTH_SESSION_STORAGE_KEY);
            openedAuthUrlSessionIdRef.current = null;
            return;
        }
        if (isAuthTerminal(activeAuthSession.status)) {
            window.localStorage.removeItem(AGENT_AUTH_SESSION_STORAGE_KEY);
            openedAuthUrlSessionIdRef.current = null;
            return;
        }
        window.localStorage.setItem(
            AGENT_AUTH_SESSION_STORAGE_KEY,
            activeAuthSession.session_id
        );
    }, [activeAuthSession?.session_id, activeAuthSession?.status]);

    // Auto-open auth URL in new tab once per session (same as Claude / Codex flow)
    useEffect(() => {
        if (!activeAuthSession?.session_id || !activeAuthSession?.action_url) return;
        if (isAuthTerminal(activeAuthSession.status)) return;
        if (openedAuthUrlSessionIdRef.current === activeAuthSession.session_id) return;
        openedAuthUrlSessionIdRef.current = activeAuthSession.session_id;
        window.open(activeAuthSession.action_url ?? '', '_blank', 'noopener,noreferrer');
    }, [activeAuthSession?.session_id, activeAuthSession?.status, activeAuthSession?.action_url]);

    const handleSaveGitLab = async () => {
        try {
            await save({
                ...settings!,
                gitlab: {
                    ...settings!.gitlab,
                    url: gitlabUrl,
                    token: gitlabToken,
                    autoSync: settings!.gitlab?.autoSync ?? true,
                    configured: true
                }
            });
            setIsEditingGitlab(false);
            showToast('GitLab settings saved successfully', 'success');
        } catch {
            showToast('Failed to save GitLab settings', 'error');
        }
    };

    const handleSaveCloudflare = async () => {
        try {
            await save({
                ...settings!,
                cloudflare: {
                    ...settings!.cloudflare,
                    accountId: cfAccountId,
                    token: cfToken,
                    zoneId: cfZoneId,
                    baseDomain: cfBaseDomain,
                    configured: true
                }
            });
            setIsEditingCloudflare(false);
            showToast('Cloudflare settings saved successfully', 'success');
        } catch {
            showToast('Failed to save Cloudflare settings', 'error');
        }
    };

    const handleSaveWorktreesAndLanguage = async () => {
        try {
            await save({
                ...settings!,
                worktreesPath: worktreesPath.trim() || './worktrees',
                preferredAgentLanguage,
            });
            setIsEditingWorktrees(false);
            showToast('Worktrees path and conversation language saved.', 'success');
        } catch {
            showToast('Failed to save settings.', 'error');
        }
    };

    const handleSaveAgent = async () => {
        const status = agentProvidersStatus?.providers.find(
            (providerStatus) => providerStatus.provider === agentProvider
        );
        if (!status?.available) {
            showToast(
                `Cannot set ${PROVIDER_LABELS[agentProvider] || agentProvider} as default: ${providerReasonLabel(status)}`,
                'error'
            );
            return;
        }

        try {
            await save({
                ...settings!,
                agent: {
                    ...settings!.agent,
                    provider: agentProvider,
                }
            });
            setIsEditingAgent(false);
            showToast('Agent settings saved successfully', 'success');
            // Refresh status after provider/key change
            await handleRefreshAgentStatus();
        } catch {
            showToast('Failed to save agent settings', 'error');
        }
    };

    const handleRefreshAgentStatus = async () => {
        setAgentLoading(true);
        try {
            const response = await getAgentProvidersStatus();
            setAgentProvidersStatus(response);
            writeCachedAgentProviderStatus(response);
            showToast('Agent status refreshed', 'success');
        } catch {
            showToast('Failed to refresh agent status', 'error');
        } finally {
            setAgentLoading(false);
        }
    };

    const handleStartProviderAuth = async (provider: string, forceReauth?: boolean) => {
        setAgentAuthLoadingProvider(provider);
        setShowAuthDialog(true);
        setActiveAuthSession(null);
        setAuthInput('');
        try {
            const session = await initiateAgentAuth(provider, forceReauth);
            setActiveAuthSession(session);
            setAuthInput('');
            setShowAuthDialog(true);
            showToast(
                `Auth session started for ${PROVIDER_LABELS[provider] || provider} (${session.session_id.slice(0, 8)}...)`,
                'success'
            );
        } catch (error) {
            const detail =
                error instanceof ApiError && error.message
                    ? `: ${error.message}`
                    : '';
            showToast(
                `Failed to start auth for ${PROVIDER_LABELS[provider] || provider}${detail}`,
                'error'
            );
            setShowAuthDialog(false);
        } finally {
            setAgentAuthLoadingProvider(null);
        }
    };

    const handleSubmitAuthInput = async () => {
        if (!activeAuthSession?.session_id) return;
        const trimmed = authInput.trim();
        if (!trimmed) {
            showToast('Please enter auth code or callback URL', 'error');
            return;
        }

        setAuthSubmitting(true);
        try {
            await submitAgentAuthCode(activeAuthSession.session_id, trimmed);
            showToast('Auth input submitted. Waiting for provider confirmation...', 'success');
            setAuthInput('');
            const latest = await getAgentAuthSession(activeAuthSession.session_id);
            setActiveAuthSession(latest);
        } catch {
            showToast('Failed to submit auth input', 'error');
        } finally {
            setAuthSubmitting(false);
        }
    };

    const handleCancelAuthSession = async () => {
        if (!activeAuthSession?.session_id) return;
        setAuthSubmitting(true);
        try {
            const updated = await cancelAgentAuth(activeAuthSession.session_id);
            setActiveAuthSession(updated);
            showToast('Auth session cancelled', 'success');
            await handleRefreshAgentStatus();
        } catch {
            showToast('Failed to cancel auth session', 'error');
        } finally {
            setAuthSubmitting(false);
        }
    };

    if (settingsLoading) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center">
                    <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary"></div>
                </div>
            </AppShell>
        );
    }

    const guide = activeGuide ? SETTINGS_GUIDES[activeGuide] : null;
    const providerStatuses = agentProvidersStatus?.providers || [];
    const selectedProviderStatus = providerStatuses.find(
        (status) => status.provider === agentProvider
    );
    const activeProviderLabel = activeAuthSession
        ? PROVIDER_LABELS[activeAuthSession.provider] || activeAuthSession.provider
        : '';
    const authIsTerminal = isAuthTerminal(activeAuthSession?.status);
    const authStatusLabel = activeAuthSession?.status
        ? activeAuthSession.status.replace(/_/g, ' ')
        : 'idle';
    const handleCloseAuthDialog = () => {
        setShowAuthDialog(false);
        setAuthInput('');
        setActiveAuthSession(null);
        handledTerminalAuthSeqRef.current = null;
        window.localStorage.removeItem(AGENT_AUTH_SESSION_STORAGE_KEY);
    };
    const isProviderSelectable = (provider: string) => {
        if (providerStatuses.length === 0) return true;
        return providerStatuses.find((status) => status.provider === provider)?.available ?? false;
    };

    return (
        <AppShell>
            <div className="flex-1 overflow-y-auto p-6 md:p-10 scroll-smooth scrollbar-hide bg-background">
                <div className="max-w-4xl mx-auto flex flex-col gap-8 pb-20">
                    <div className="flex flex-col gap-2">
                        <h1 className="text-card-foreground text-3xl font-bold leading-tight">Settings & Integrations</h1>
                        <p className="text-muted-foreground text-base font-normal">Manage source control, agent intelligence, and deployment configurations.</p>
                    </div>

                    {/* Worktrees Path & Agent Conversation Language - same row */}
                    <section className="flex flex-col gap-4">
                        <div className="flex items-center justify-between border-b border-border pb-2">
                            <h3 className="text-card-foreground text-lg font-bold leading-tight">Worktrees Path & Agent Language</h3>
                            {!isEditingWorktrees ? (
                                <button
                                    onClick={() => setIsEditingWorktrees(true)}
                                    className="p-2 text-muted-foreground hover:text-primary hover:bg-muted rounded-lg transition-colors"
                                    title="Edit"
                                >
                                    <span className="material-symbols-outlined">edit</span>
                                </button>
                            ) : (
                                <div className="flex items-center gap-2">
                                    <button
                                        onClick={() => {
                                            setIsEditingWorktrees(false);
                                            setWorktreesPath(settings?.worktreesPath || './worktrees');
                                            setPreferredAgentLanguage((settings?.preferredAgentLanguage === 'vi' ? 'vi' : 'en') as 'en' | 'vi');
                                        }}
                                        className="px-3 py-1.5 text-xs font-bold text-muted-foreground hover:text-card-foreground transition-colors"
                                    >
                                        Cancel
                                    </button>
                                    <button
                                        onClick={handleSaveWorktreesAndLanguage}
                                        disabled={saving}
                                        className="px-3 py-1.5 bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-bold rounded-lg shadow-sm transition-colors flex items-center gap-1 disabled:opacity-50 disabled:cursor-not-allowed"
                                    >
                                        {saving ? (
                                            <>
                                                <span className="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                                                Saving...
                                            </>
                                        ) : (
                                            <>
                                                <span className="material-symbols-outlined text-[14px]">save</span>
                                                Save
                                            </>
                                        )}
                                    </button>
                                </div>
                            )}
                        </div>
                        {isEditingWorktrees ? (
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <div className="space-y-2">
                                    <label className="text-sm font-medium text-card-foreground">Worktrees path</label>
                                    <input
                                        type="text"
                                        value={worktreesPath}
                                        onChange={(e) => setWorktreesPath(e.target.value)}
                                        placeholder="./worktrees"
                                        className="w-full px-4 py-2 bg-muted border border-border rounded-lg text-card-foreground font-mono text-sm focus:ring-1 focus:ring-primary focus:border-primary placeholder:text-muted-foreground"
                                    />
                                    <p className="text-xs text-muted-foreground">
                                        Directory for cloned source code. Applied immediately on save.
                                    </p>
                                </div>
                                <div className="space-y-2">
                                    <label className="text-sm font-medium text-card-foreground">Conversation language</label>
                                    <select
                                        value={preferredAgentLanguage}
                                        onChange={(e) => setPreferredAgentLanguage(e.target.value as 'en' | 'vi')}
                                        className="w-full px-4 py-2 bg-muted border border-border rounded-lg text-card-foreground text-sm focus:ring-1 focus:ring-primary focus:border-primary"
                                    >
                                        <option value="en">English</option>
                                        <option value="vi">Tiếng Việt</option>
                                    </select>
                                    <p className="text-xs text-muted-foreground">
                                        Language the agent uses when replying in chat and task runs.
                                    </p>
                                </div>
                            </div>
                        ) : (
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <div className="flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-3">
                                    <span className="material-symbols-outlined text-muted-foreground text-lg shrink-0">folder</span>
                                    <span className="font-mono text-sm text-card-foreground break-all">{settings?.worktreesPath || './worktrees'}</span>
                                </div>
                                <div className="flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-3">
                                    <span className="material-symbols-outlined text-muted-foreground text-lg shrink-0">language</span>
                                    <span className="text-sm text-card-foreground">
                                        {settings?.preferredAgentLanguage === 'vi' ? 'Tiếng Việt' : 'English'}
                                    </span>
                                </div>
                            </div>
                        )}
                    </section>

                    {/* 1. Source Control (GitLab Self-Hosted) */}
                    <section className="flex flex-col gap-4">
                        <div className="flex items-center justify-between border-b border-border pb-2">
                            <h3 className="text-card-foreground text-lg font-bold leading-tight">Source Control</h3>
                            <SettingsGuideButton
                                onClick={() => setActiveGuide('source_control')}
                                title="How to get GitLab/GitHub source control config"
                            />
                        </div>

                        <div className="flex flex-col md:flex-row items-stretch gap-6 rounded-xl bg-card p-6 shadow-sm border border-border">
                            <div className="size-14 rounded-lg bg-[#FC6D26]/10 flex items-center justify-center flex-shrink-0">
                                <span className="material-symbols-outlined text-[#FC6D26] text-3xl">code</span>
                            </div>
                            <div className="flex flex-col flex-1 gap-4">
                                <div className="flex justify-between items-start">
                                    <div>
                                        <h4 className="text-card-foreground text-lg font-bold">GitLab or GitHub</h4>
                                        <p className="text-muted-foreground text-sm mt-1">Configure one provider: URL (gitlab.com or github.com) + PAT for import and private clone.</p>
                                    </div>

                                    <div className="flex items-center gap-3">
                                        {!isEditingGitlab ? (
                                            <>
                                                {settings?.gitlab?.configured ? (
                                                    <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-green-500/10 dark:bg-green-500/20 border border-green-500/20 dark:border-green-500/30">
                                                        <span className="material-symbols-outlined text-green-500 dark:text-green-400 text-sm">check_circle</span>
                                                        <span className="text-xs font-bold text-green-500 dark:text-green-400 uppercase">Connected</span>
                                                    </div>
                                                ) : (
                                                    <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-muted border border-border">
                                                        <span className="material-symbols-outlined text-muted-foreground text-sm">link_off</span>
                                                        <span className="text-xs font-bold text-muted-foreground uppercase">Not Configured</span>
                                                    </div>
                                                )}
                                                <button
                                                    onClick={() => setIsEditingGitlab(true)}
                                                    className="p-2 text-muted-foreground hover:text-primary hover:bg-muted rounded-lg transition-colors"
                                                    title="Edit Configuration"
                                                >
                                                    <span className="material-symbols-outlined">edit</span>
                                                </button>
                                            </>
                                        ) : (
                                            <div className="flex items-center gap-2">
                                                <button
                                                    onClick={() => setIsEditingGitlab(false)}
                                                    className="px-3 py-1.5 text-xs font-bold text-muted-foreground hover:text-card-foreground transition-colors"
                                                >
                                                    Cancel
                                                </button>
                                                <button
                                                    onClick={handleSaveGitLab}
                                                    disabled={saving}
                                                    className="px-3 py-1.5 bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-bold rounded-lg shadow-sm transition-colors flex items-center gap-1 disabled:opacity-50 disabled:cursor-not-allowed"
                                                >
                                                    {saving ? (
                                                        <>
                                                            <span className="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                                                            Saving...
                                                        </>
                                                    ) : (
                                                        <>
                                                            <span className="material-symbols-outlined text-[14px]">save</span>
                                                            Save
                                                        </>
                                                    )}
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                </div>

                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mt-2">
                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Instance URL (gitlab.com or github.com)</label>
                                        <div className="relative">
                                            <span className={`absolute left-3 top-1/2 -translate-y-1/2 text-[18px] transition-colors ${isEditingGitlab ? 'text-muted-foreground' : 'text-muted-foreground/50'} material-symbols-outlined`}>link</span>
                                            <input
                                                className={`block w-full pl-10 p-2.5 text-sm rounded-lg border font-mono transition-all ${isEditingGitlab
                                                    ? 'bg-muted border-border text-card-foreground focus:ring-primary focus:border-primary'
                                                    : 'bg-muted/50 border-transparent text-muted-foreground cursor-not-allowed'
                                                    }`}
                                                type="text"
                                                value={gitlabUrl}
                                                disabled={!isEditingGitlab}
                                                onChange={(e) => setGitlabUrl(e.target.value)}
                                                placeholder="https://gitlab.com or https://github.com"
                                            />
                                        </div>
                                    </div>
                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Personal Access Token</label>
                                        <div className="relative">
                                            <span className={`absolute left-3 top-1/2 -translate-y-1/2 text-[18px] transition-colors ${isEditingGitlab ? 'text-muted-foreground' : 'text-muted-foreground/50'} material-symbols-outlined`}>key</span>
                                            <input
                                                className={`block w-full pl-10 pr-10 p-2.5 text-sm rounded-lg border font-mono transition-all ${isEditingGitlab
                                                    ? 'bg-muted border-border text-card-foreground focus:ring-primary focus:border-primary'
                                                    : 'bg-muted/50 border-transparent text-muted-foreground cursor-not-allowed'
                                                    }`}
                                                type={showGitlabToken && isEditingGitlab ? "text" : "password"}
                                                value={gitlabToken}
                                                disabled={!isEditingGitlab}
                                                onChange={(e) => setGitlabToken(e.target.value)}
                                                placeholder="glpat-... or ghp_..."
                                            />
                                            {isEditingGitlab && (
                                                <button
                                                    onClick={() => setShowGitlabToken(!showGitlabToken)}
                                                    className="absolute inset-y-0 right-0 flex items-center pr-3 text-muted-foreground hover:text-card-foreground transition-colors"
                                                >
                                                    <span className="material-symbols-outlined text-lg">{showGitlabToken ? 'visibility' : 'visibility_off'}</span>
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* 2. Agent CLI Provider */}
                    <section className="flex flex-col gap-4">
                        <div className="flex items-center justify-between border-b border-border pb-2">
                            <div className="flex items-center gap-2">
                                <span className="material-symbols-outlined text-primary">psychology</span>
                                <h3 className="text-card-foreground text-lg font-bold leading-tight">Agent CLI Provider</h3>
                            </div>
                            <SettingsGuideButton
                                onClick={() => setActiveGuide('agent_auth')}
                                title="How to authenticate AI agent providers"
                            />
                        </div>

                        <div className="rounded-xl bg-card p-6 shadow-sm border border-border ring-1 ring-primary/30 relative overflow-hidden">
                            <div className="absolute top-0 right-0 w-32 h-32 bg-primary/5 rounded-full blur-3xl -mr-10 -mt-10"></div>
                            <div className="flex flex-col gap-6 relative z-10">
                                <div className="flex justify-between items-start gap-4">
                                    <div>
                                        <h4 className="text-card-foreground text-lg font-bold">Agent Execution Runtime</h4>
                                        <p className="text-muted-foreground text-sm mt-1">
                                            Choose which local CLI executes agent attempts (Claude Code, OpenAI Codex, or Google Gemini).
                                        </p>
                                    </div>

                                    <div className="flex items-center gap-3">
                                        <button
                                            onClick={handleRefreshAgentStatus}
                                            disabled={agentLoading}
                                            className="p-1.5 text-muted-foreground hover:text-primary hover:bg-muted rounded-lg transition-colors disabled:opacity-50"
                                            title="Refresh Status"
                                        >
                                            <span className={`material-symbols-outlined text-lg ${agentLoading ? 'animate-spin' : ''}`}>refresh</span>
                                        </button>

                                        {!isEditingAgent ? (
                                            <button
                                                onClick={() => setIsEditingAgent(true)}
                                                className="p-2 text-muted-foreground hover:text-primary hover:bg-muted rounded-lg transition-colors"
                                                title="Edit Agent Configuration"
                                            >
                                                <span className="material-symbols-outlined">edit</span>
                                            </button>
                                        ) : (
                                            <div className="flex items-center gap-2">
                                                <button
                                                    onClick={() => setIsEditingAgent(false)}
                                                    className="px-3 py-1.5 text-xs font-bold text-muted-foreground hover:text-card-foreground transition-colors"
                                                >
                                                    Cancel
                                                </button>
                                                <button
                                                    onClick={handleSaveAgent}
                                                    disabled={
                                                        saving ||
                                                        (providerStatuses.length > 0 &&
                                                            !selectedProviderStatus?.available)
                                                    }
                                                    className="px-3 py-1.5 bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-bold rounded-lg shadow-sm transition-colors flex items-center gap-1 disabled:opacity-50 disabled:cursor-not-allowed"
                                                >
                                                    {saving ? (
                                                        <>
                                                            <span className="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                                                            Saving...
                                                        </>
                                                    ) : (
                                                        <>
                                                            <span className="material-symbols-outlined text-[14px]">save</span>
                                                            Save
                                                        </>
                                                    )}
                                                </button>
                                            </div>
                                        )}

                                        {agentLoading ? (
                                            <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-muted border border-border">
                                                <span className="material-symbols-outlined text-muted-foreground text-sm animate-spin">progress_activity</span>
                                                <span className="text-xs font-bold text-muted-foreground uppercase">Checking...</span>
                                            </div>
                                        ) : selectedProviderStatus?.available ? (
                                            <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-green-500/10 dark:bg-green-500/20 border border-green-500/20 dark:border-green-500/30">
                                                <span className="material-symbols-outlined text-green-500 dark:text-green-400 text-sm">check_circle</span>
                                                <span className="text-xs font-bold text-green-500 dark:text-green-400 uppercase">Available</span>
                                            </div>
                                        ) : (
                                            <div className="flex items-center gap-2 px-3 py-1 rounded-full bg-amber-500/10 dark:bg-amber-500/20 border border-amber-500/20 dark:border-amber-500/30">
                                                <span className="material-symbols-outlined text-amber-500 dark:text-amber-400 text-sm">warning</span>
                                                <span className="text-xs font-bold text-amber-500 dark:text-amber-400 uppercase">Not Available</span>
                                            </div>
                                        )}
                                    </div>
                                </div>

                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Provider</label>
                                        <div className="relative">
                                            <span className={`absolute left-3 top-1/2 -translate-y-1/2 text-[18px] transition-colors ${isEditingAgent ? 'text-muted-foreground' : 'text-muted-foreground/50'} material-symbols-outlined`}>hub</span>
                                            <select
                                                className={`block w-full pl-10 p-2.5 text-sm rounded-lg border font-mono transition-all ${isEditingAgent
                                                    ? 'bg-muted border-border text-card-foreground focus:ring-primary focus:border-primary'
                                                    : 'bg-muted/50 border-transparent text-muted-foreground cursor-not-allowed'
                                                    }`}
                                                value={agentProvider}
                                                disabled={!isEditingAgent}
                                                onChange={(e) => setAgentProvider(e.target.value)}
                                            >
                                                <option
                                                    value="claude-code"
                                                    disabled={isEditingAgent && !isProviderSelectable('claude-code')}
                                                >
                                                    Claude Code (Anthropic)
                                                </option>
                                                <option
                                                    value="openai-codex"
                                                    disabled={isEditingAgent && !isProviderSelectable('openai-codex')}
                                                >
                                                    Codex CLI (OpenAI)
                                                </option>
                                                <option
                                                    value="gemini-cli"
                                                    disabled={isEditingAgent && !isProviderSelectable('gemini-cli')}
                                                >
                                                    Gemini CLI (Google)
                                                </option>
                                                <option
                                                    value="cursor-cli"
                                                    disabled={isEditingAgent && !isProviderSelectable('cursor-cli')}
                                                >
                                                    Cursor CLI
                                                </option>
                                            </select>
                                        </div>
                                    </div>

                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Status</label>
                                        <div className="bg-muted/50 border border-border rounded-lg p-3">
                                            <p className="text-sm text-card-foreground font-medium">
                                                {selectedProviderStatus?.message || 'Provider status unavailable'}
                                            </p>
                                            <p className="text-xs text-muted-foreground mt-1 font-mono">
                                                Provider: <span className="text-card-foreground">{PROVIDER_LABELS[agentProvider] || agentProvider}</span>
                                            </p>
                                            {!selectedProviderStatus?.available && (
                                                <p className="text-xs text-amber-500 dark:text-amber-400 mt-1 font-mono">
                                                    Reason: {providerReasonLabel(selectedProviderStatus)}
                                                </p>
                                            )}
                                        </div>
                                    </div>
                                </div>

                                <div className="bg-muted/50 border border-border rounded-lg p-3">
                                    <p className="text-sm text-card-foreground font-medium">
                                        Authentication is managed via the selected CLI on this server.
                                    </p>
                                    <p className="text-xs text-muted-foreground mt-1 font-mono">
                                        Available = CLI installed + credentials valid for the provider.
                                    </p>
                                </div>

                                {!selectedProviderStatus?.available && (
                                    <div className="bg-amber-500/10 border border-amber-500/30 rounded-lg p-3">
                                        <p className="text-sm text-amber-500 dark:text-amber-400 font-medium">
                                            Selected default provider is not available.
                                        </p>
                                        <p className="text-xs text-amber-500/90 dark:text-amber-300 mt-1 font-mono">
                                            Choose an available provider as default, or click Sign in/Re-auth for this provider.
                                        </p>
                                    </div>
                                )}

                                <div className="space-y-2">
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Provider Availability</label>
                                    {(['claude-code', 'openai-codex', 'gemini-cli', 'cursor-cli'] as const).map((providerKey) => {
                                        const providerStatus = providerStatuses.find(
                                            (status) => status.provider === providerKey
                                        );
                                        const isAvailable = providerStatus?.available || false;
                                        const isDefault = agentProvider === providerKey;
                                        const actionLoading = agentAuthLoadingProvider === providerKey;

                                        return (
                                            <div
                                                key={providerKey}
                                                className={`rounded-lg border p-3 ${isDefault ? 'border-primary/40 bg-primary/5' : 'border-border bg-muted/40'
                                                    }`}
                                            >
                                                <div className="flex items-start justify-between gap-3">
                                                    <div className="min-w-0">
                                                        <div className="flex items-center gap-2">
                                                            <p className="text-sm font-semibold text-card-foreground">
                                                                {PROVIDER_LABELS[providerKey]}
                                                            </p>
                                                            {isDefault && (
                                                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-primary/20 text-primary font-bold uppercase tracking-wide">
                                                                    Default
                                                                </span>
                                                            )}
                                                        </div>
                                                        <p className="text-xs text-muted-foreground mt-1">
                                                            {isAvailable ? 'Available' : providerReasonLabel(providerStatus)}
                                                        </p>
                                                    </div>

                                                    <div className="flex items-center gap-2 shrink-0">
                                                        <span className={`text-[10px] px-2 py-1 rounded-full border font-bold uppercase tracking-wide ${isAvailable
                                                            ? 'text-green-500 dark:text-green-400 border-green-500/30 bg-green-500/10'
                                                            : 'text-amber-500 dark:text-amber-400 border-amber-500/30 bg-amber-500/10'
                                                            }`}>
                                                            {isAvailable ? 'Available' : 'Not available'}
                                                        </span>
                                                        <button
                                                            type="button"
                                                            onClick={() =>
                                                                handleStartProviderAuth(
                                                                    providerKey,
                                                                    (providerKey === 'gemini-cli' || providerKey === 'cursor-cli') && isAvailable
                                                                )
                                                            }
                                                            disabled={actionLoading}
                                                            className="px-2.5 py-1.5 text-xs font-bold rounded-md border border-border bg-muted hover:bg-muted/80 text-card-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                                        >
                                                            {actionLoading ? 'Starting...' : isAvailable ? 'Re-auth' : 'Sign in'}
                                                        </button>
                                                    </div>
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* 3. Cloudflare Deployment */}
                    <section className="flex flex-col gap-4">
                        <div className="flex items-center justify-between border-b border-border pb-2">
                            <div className="flex items-center gap-2">
                                <span className="material-symbols-outlined text-[#F38020]">cloud</span>
                                <h3 className="text-card-foreground text-lg font-bold leading-tight">Deployment Provider</h3>
                            </div>
                            <SettingsGuideButton
                                onClick={() => setActiveGuide('cloudflare')}
                                title="How to get Cloudflare deployment config"
                            />
                        </div>

                        <div className="flex flex-col md:flex-row items-stretch gap-6 rounded-xl bg-card p-6 shadow-sm border border-border">
                            <div className="size-14 rounded-lg bg-[#F38020]/10 flex items-center justify-center flex-shrink-0">
                                <span className="material-symbols-outlined text-[#F38020] text-3xl">cloud</span>
                            </div>
                            <div className="flex flex-col flex-1 gap-4">
                                <div className="flex justify-between items-start">
                                    <div>
                                        <h4 className="text-card-foreground text-lg font-bold">Cloudflare</h4>
                                        <p className="text-muted-foreground text-sm mt-1">Connect to Cloudflare Pages & Workers for deployment.</p>
                                    </div>

                                    <div className="flex items-center gap-3">
                                        {!isEditingCloudflare ? (
                                            <button
                                                onClick={() => setIsEditingCloudflare(true)}
                                                className="p-2 text-muted-foreground hover:text-primary hover:bg-muted rounded-lg transition-colors"
                                                title="Edit Configuration"
                                            >
                                                <span className="material-symbols-outlined">edit</span>
                                            </button>
                                        ) : (
                                            <div className="flex items-center gap-2">
                                                <button
                                                    onClick={() => setIsEditingCloudflare(false)}
                                                    className="px-3 py-1.5 text-xs font-bold text-muted-foreground hover:text-card-foreground transition-colors"
                                                >
                                                    Cancel
                                                </button>
                                                <button
                                                    onClick={handleSaveCloudflare}
                                                    disabled={saving}
                                                    className="px-3 py-1.5 bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-bold rounded-lg shadow-sm transition-colors flex items-center gap-1 disabled:opacity-50 disabled:cursor-not-allowed"
                                                >
                                                    {saving ? (
                                                        <>
                                                            <span className="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                                                            Saving...
                                                        </>
                                                    ) : (
                                                        <>
                                                            <span className="material-symbols-outlined text-[14px]">save</span>
                                                            Save
                                                        </>
                                                    )}
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                </div>

                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mt-2">
                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Account ID</label>
                                        <div className="relative">
                                            <span className={`absolute left-3 top-1/2 -translate-y-1/2 text-[18px] transition-colors ${isEditingCloudflare ? 'text-muted-foreground' : 'text-muted-foreground/50'} material-symbols-outlined`}>badge</span>
                                            <input
                                                className={`block w-full pl-10 p-2.5 text-sm rounded-lg border font-mono transition-all ${isEditingCloudflare
                                                    ? 'bg-muted border-border text-card-foreground focus:ring-primary focus:border-primary'
                                                    : 'bg-muted/50 border-transparent text-muted-foreground cursor-not-allowed'
                                                    }`}
                                                type="text"
                                                value={cfAccountId}
                                                disabled={!isEditingCloudflare}
                                                onChange={(e) => setCfAccountId(e.target.value)}
                                                placeholder="e.g. 8f2a1c..."
                                            />
                                        </div>
                                    </div>
                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">API Token</label>
                                        <div className="relative">
                                            <span className={`absolute left-3 top-1/2 -translate-y-1/2 text-[18px] transition-colors ${isEditingCloudflare ? 'text-muted-foreground' : 'text-muted-foreground/50'} material-symbols-outlined`}>api</span>
                                            <input
                                                className={`block w-full pl-10 pr-10 p-2.5 text-sm rounded-lg border font-mono transition-all ${isEditingCloudflare
                                                    ? 'bg-muted border-border text-card-foreground focus:ring-primary focus:border-primary'
                                                    : 'bg-muted/50 border-transparent text-muted-foreground cursor-not-allowed'
                                                    }`}
                                                type={showCfToken && isEditingCloudflare ? "text" : "password"}
                                                value={cfToken}
                                                disabled={!isEditingCloudflare}
                                                onChange={(e) => setCfToken(e.target.value)}
                                                placeholder="Cloudflare API Token"
                                            />
                                            {isEditingCloudflare && (
                                                <button
                                                    onClick={() => setShowCfToken(!showCfToken)}
                                                    className="absolute inset-y-0 right-0 flex items-center pr-3 text-muted-foreground hover:text-card-foreground transition-colors"
                                                >
                                                    <span className="material-symbols-outlined text-lg">{showCfToken ? 'visibility' : 'visibility_off'}</span>
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Zone ID</label>
                                        <div className="relative">
                                            <span className={`absolute left-3 top-1/2 -translate-y-1/2 text-[18px] transition-colors ${isEditingCloudflare ? 'text-muted-foreground' : 'text-muted-foreground/50'} material-symbols-outlined`}>dns</span>
                                            <input
                                                className={`block w-full pl-10 p-2.5 text-sm rounded-lg border font-mono transition-all ${isEditingCloudflare
                                                    ? 'bg-muted border-border text-card-foreground focus:ring-primary focus:border-primary'
                                                    : 'bg-muted/50 border-transparent text-muted-foreground cursor-not-allowed'
                                                    }`}
                                                type="text"
                                                value={cfZoneId}
                                                disabled={!isEditingCloudflare}
                                                onChange={(e) => setCfZoneId(e.target.value)}
                                                placeholder="e.g. 023e105..."
                                            />
                                        </div>
                                    </div>
                                    <div className="flex flex-col gap-1.5">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Base Domain</label>
                                        <div className="relative">
                                            <span className={`absolute left-3 top-1/2 -translate-y-1/2 text-[18px] transition-colors ${isEditingCloudflare ? 'text-muted-foreground' : 'text-muted-foreground/50'} material-symbols-outlined`}>public</span>
                                            <input
                                                className={`block w-full pl-10 p-2.5 text-sm rounded-lg border font-mono transition-all ${isEditingCloudflare
                                                    ? 'bg-muted border-border text-card-foreground focus:ring-primary focus:border-primary'
                                                    : 'bg-muted/50 border-transparent text-muted-foreground cursor-not-allowed'
                                                    }`}
                                                type="text"
                                                value={cfBaseDomain}
                                                disabled={!isEditingCloudflare}
                                                onChange={(e) => setCfBaseDomain(e.target.value)}
                                                placeholder="e.g. previews.acpms.dev"
                                            />
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </section>

                </div>
            </div>

            {/* Toast notifications */}
            {toasts.map((toast) => (
                <Toast
                    key={toast.id}
                    message={toast.message}
                    type={toast.type}
                    onClose={() => hideToast(toast.id)}
                />
            ))}

            <Dialog open={activeGuide !== null} onOpenChange={(open) => !open && setActiveGuide(null)}>
                <DialogContent className="max-w-3xl p-0 overflow-hidden">
                    {guide && (
                        <div className="flex flex-col">
                            <div className="px-6 py-5 border-b border-border bg-gradient-to-r from-primary/[0.08] via-primary/[0.03] to-transparent">
                                <div className="inline-flex items-center gap-2 rounded-full bg-background/80 border border-border px-3 py-1">
                                    <span className="material-symbols-outlined text-sm text-primary">tips_and_updates</span>
                                    <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                                        {guide.audienceLabel}
                                    </span>
                                </div>
                                <DialogHeader className="mt-3 space-y-2">
                                    <DialogTitle className="text-xl text-card-foreground">{guide.title}</DialogTitle>
                                    <DialogDescription className="text-sm text-muted-foreground">
                                        {guide.description}
                                    </DialogDescription>
                                </DialogHeader>
                            </div>

                            <div className="p-6 space-y-5">
                                <div className="rounded-xl border border-border bg-muted/40 p-4">
                                    <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Before You Start</p>
                                    <div className="mt-3 flex flex-wrap gap-2">
                                        {guide.prep.map((item) => (
                                            <span
                                                key={item}
                                                className="inline-flex items-center rounded-full border border-border bg-background px-3 py-1 text-xs text-card-foreground"
                                            >
                                                {item}
                                            </span>
                                        ))}
                                    </div>
                                </div>

                                <div className="grid gap-3">
                                    {guide.steps.map((step, index) => (
                                        <div key={step.title} className="rounded-xl border border-border bg-card p-4 shadow-sm">
                                            <div className="flex items-start gap-3">
                                                <div className="size-8 rounded-full bg-primary text-primary-foreground text-sm font-bold flex items-center justify-center shrink-0">
                                                    {index + 1}
                                                </div>
                                                <div className="size-8 rounded-lg bg-primary/10 text-primary flex items-center justify-center shrink-0">
                                                    <span className="material-symbols-outlined text-[18px]">{step.icon}</span>
                                                </div>
                                                <div className="flex-1 min-w-0">
                                                    <p className="text-[11px] font-semibold uppercase tracking-wide text-primary">{step.label}</p>
                                                    <p className="text-sm font-semibold text-card-foreground mt-0.5">{step.title}</p>
                                                    <p className="text-sm text-muted-foreground mt-1">{step.detail}</p>
                                                    {step.hint && (
                                                        <p className="mt-2 text-xs text-muted-foreground bg-muted/60 rounded-md px-2 py-1">
                                                            {step.hint}
                                                        </p>
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                    ))}
                                </div>

                                {guide.note && (
                                    <div className="rounded-xl border border-amber-400/30 bg-amber-500/10 px-4 py-3 text-sm text-amber-700 dark:text-amber-300">
                                        <span className="font-semibold">Note:</span> {guide.note}
                                    </div>
                                )}
                            </div>
                        </div>
                    )}
                </DialogContent>
            </Dialog>

            <Dialog
                open={showAuthDialog}
                onOpenChange={(open) => {
                    if (open) {
                        setShowAuthDialog(true);
                        return;
                    }
                    handleCloseAuthDialog();
                }}
            >
                <DialogContent className="w-[min(92vw,42rem)] max-w-[42rem] overflow-hidden">
                    <DialogHeader>
                        <DialogTitle>Provider Authentication</DialogTitle>
                        <DialogDescription>
                            Complete auth flow for {activeProviderLabel || 'selected provider'}.
                        </DialogDescription>
                    </DialogHeader>

                    {activeAuthSession ? (
                        <div className="space-y-4 min-w-0">
                            {activeAuthSession.status === 'succeeded' && (
                                <div className="rounded-lg border border-green-500/50 bg-green-500/10 p-4 flex items-center gap-3">
                                    <span className="material-symbols-outlined text-3xl text-green-600 dark:text-green-400">check_circle</span>
                                    <div>
                                        <p className="text-sm font-semibold text-green-700 dark:text-green-300">
                                            Authentication successful
                                        </p>
                                        <p className="text-xs text-green-600/90 dark:text-green-400/90 mt-0.5">
                                            {activeProviderLabel} is ready. You can close this dialog.
                                        </p>
                                    </div>
                                </div>
                            )}
                            <div className="rounded-lg border border-border bg-muted/40 p-3">
                                <p className="text-sm text-card-foreground font-semibold">
                                    Session {activeAuthSession.session_id.slice(0, 8)}...
                                </p>
                                <p className="mt-1 text-xs text-muted-foreground uppercase tracking-wide">
                                    Status: {authStatusLabel}
                                </p>
                                {activeAuthSession.last_error && (
                                    <p className="mt-2 text-xs text-red-500 dark:text-red-400 break-words">
                                        {activeAuthSession.last_error}
                                    </p>
                                )}
                                {activeAuthSession.action_hint && (
                                    <p className="mt-2 text-xs text-muted-foreground break-words">
                                        {activeAuthSession.action_hint}
                                    </p>
                                )}
                            </div>

                            {activeAuthSession.action_url && (
                                <div className="rounded-lg border border-border bg-card p-3 flex items-start gap-3 overflow-hidden">
                                    <div className="min-w-0 flex-1 overflow-hidden">
                                        <p className="text-xs uppercase tracking-wide text-muted-foreground">
                                            Auth URL
                                        </p>
                                        <p
                                            className="mt-1 block w-full overflow-hidden text-ellipsis whitespace-nowrap text-sm text-card-foreground"
                                            title={activeAuthSession.action_url}
                                        >
                                            {activeAuthSession.action_url}
                                        </p>
                                    </div>
                                    <a
                                        href={activeAuthSession.action_url}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="shrink-0 px-3 py-1.5 text-xs font-semibold rounded-md border border-border bg-muted hover:bg-muted/80 transition-colors"
                                    >
                                        Open Link
                                    </a>
                                </div>
                            )}

                            {activeAuthSession.action_code && (
                                <div className="rounded-lg border border-border bg-card p-3">
                                    <p className="text-xs uppercase tracking-wide text-muted-foreground">
                                        Device / one-time code
                                    </p>
                                    <div className="mt-1 flex items-center gap-1.5 flex-wrap">
                                        <span className="text-sm font-mono text-card-foreground break-all">
                                            {activeAuthSession.action_code}
                                        </span>
                                        <button
                                            type="button"
                                            onClick={async () => {
                                                try {
                                                    await navigator.clipboard.writeText(activeAuthSession.action_code ?? '');
                                                    setDeviceCodeCopied(true);
                                                    setTimeout(() => setDeviceCodeCopied(false), 2000);
                                                } catch {
                                                    showToast('Failed to copy', 'error');
                                                }
                                            }}
                                            className="shrink-0 p-1 rounded text-muted-foreground hover:bg-muted/60 hover:text-card-foreground transition-colors"
                                            title="Copy device code"
                                            aria-label="Copy device code"
                                        >
                                            <span className="material-symbols-outlined text-[18px]">content_copy</span>
                                        </button>
                                    </div>
                                    {deviceCodeCopied && (
                                        <p className="mt-1.5 text-xs text-green-600 dark:text-green-400">
                                            Copied!
                                        </p>
                                    )}
                                </div>
                            )}

                            {!authIsTerminal && (
                                <div className="space-y-2">
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                                        Enter auth code or localhost callback URL
                                    </label>
                                    <textarea
                                        className="w-full min-h-[86px] rounded-lg border border-border bg-background p-3 text-sm text-card-foreground font-mono resize-y"
                                        value={authInput}
                                        onChange={(e) => setAuthInput(e.target.value)}
                                        placeholder="4/0AeaY... or http://127.0.0.1:port/?code=..."
                                    />
                                </div>
                            )}

                            <div className="flex items-center justify-between gap-3 pt-1">
                                <button
                                    type="button"
                                    onClick={handleCancelAuthSession}
                                    disabled={authSubmitting || authIsTerminal}
                                    className="px-3 py-1.5 text-xs font-semibold rounded-md border border-border bg-muted hover:bg-muted/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                >
                                    Cancel Session
                                </button>
                                <div className="flex items-center gap-2">
                                    <button
                                        type="button"
                                        onClick={handleCloseAuthDialog}
                                        className="px-3 py-1.5 text-xs font-semibold rounded-md border border-border bg-card hover:bg-muted/30 transition-colors"
                                    >
                                        Close
                                    </button>
                                    <button
                                        type="button"
                                        onClick={handleSubmitAuthInput}
                                        disabled={authSubmitting || authIsTerminal || !authInput.trim()}
                                        className="px-3 py-1.5 text-xs font-semibold rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                    >
                                        {authSubmitting ? 'Submitting...' : 'Submit Input'}
                                    </button>
                                </div>
                            </div>
                        </div>
                    ) : (
                        <div className="rounded-lg border border-border bg-muted/40 p-3 text-sm text-muted-foreground">
                            {agentAuthLoadingProvider ? (
                                <div className="flex items-center gap-2">
                                    <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-muted-foreground/40 border-t-muted-foreground" />
                                    <span>
                                        Starting auth session for{' '}
                                        {PROVIDER_LABELS[agentAuthLoadingProvider] ||
                                            agentAuthLoadingProvider}
                                        ...
                                    </span>
                                </div>
                            ) : (
                                'No active auth session.'
                            )}
                        </div>
                    )}
                </DialogContent>
            </Dialog>
        </AppShell>
    );
}
