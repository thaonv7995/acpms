import { useEffect, useMemo, useState } from 'react';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from '../ui/dialog';
import { ApiError } from '../../api/client';
import { useOpenClawAccess } from '../../hooks/useOpenClawAccess';
import { ConfirmModal } from './ConfirmModal';

interface OpenClawAccessModalProps {
    isOpen: boolean;
    onClose: () => void;
    showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

type StatusFilter = 'all' | 'active' | 'disabled' | 'revoked' | 'waiting_connection';

type PendingClientAction =
    | {
          clientId: string;
          clientName: string;
          action: 'disable' | 'delete';
          title: string;
          message: string;
          confirmText: string;
          confirmVariant: 'danger' | 'primary';
      }
    | null;

function formatDateTime(value: string | null): string {
    if (!value) return 'Never';
    return new Date(value).toLocaleString();
}

function compactFingerprint(value: string): string {
    if (value.length <= 22) return value;
    return `${value.slice(0, 12)}...${value.slice(-6)}`;
}

function formatRelativeTime(value: string | null): string {
    if (!value) return 'No check-in yet';

    const diffMs = Math.max(0, Date.now() - new Date(value).getTime());
    const diffMinutes = Math.floor(diffMs / (1000 * 60));

    if (diffMinutes < 1) return 'Just now';
    if (diffMinutes < 60) return `${diffMinutes}m ago`;

    const diffHours = Math.floor(diffMinutes / 60);
    if (diffHours < 24) return `${diffHours}h ago`;

    const diffDays = Math.floor(diffHours / 24);
    return `${diffDays}d ago`;
}

function getClientHealth(value: string | null): {
    label: string;
    detail: string;
    classes: string;
} {
    if (!value) {
        return {
            label: 'Never seen',
            detail: 'No runtime check-in recorded yet',
            classes: 'border-border bg-muted text-muted-foreground',
        };
    }

    const diffMs = Math.max(0, Date.now() - new Date(value).getTime());
    const fifteenMinutes = 15 * 60 * 1000;
    const twentyFourHours = 24 * 60 * 60 * 1000;

    if (diffMs <= fifteenMinutes) {
        return {
            label: 'Live now',
            detail: formatRelativeTime(value),
            classes:
                'border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
        };
    }

    if (diffMs <= twentyFourHours) {
        return {
            label: 'Recent',
            detail: formatRelativeTime(value),
            classes:
                'border-sky-500/25 bg-sky-500/10 text-sky-700 dark:text-sky-300',
        };
    }

    return {
        label: 'Stale',
        detail: formatRelativeTime(value),
        classes:
            'border-amber-500/25 bg-amber-500/10 text-amber-700 dark:text-amber-300',
    };
}

function statusPillClasses(status: string): string {
    switch (status) {
        case 'active':
            return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300 border-emerald-500/25';
        case 'disabled':
            return 'bg-amber-500/15 text-amber-700 dark:text-amber-300 border-amber-500/25';
        case 'revoked':
            return 'bg-rose-500/15 text-rose-700 dark:text-rose-300 border-rose-500/25';
        case 'waiting_connection':
            return 'bg-sky-500/15 text-sky-700 dark:text-sky-300 border-sky-500/25';
        default:
            return 'bg-muted text-muted-foreground border-border';
    }
}

function formatStatusLabel(status: string): string {
    if (status === 'waiting_connection') return 'waiting connection';
    return status.replace(/_/g, ' ');
}

function resolveErrorMessage(error: unknown, fallback: string): string {
    if (error instanceof ApiError && error.message) return error.message;
    if (error instanceof Error && error.message) return error.message;
    return fallback;
}

export function OpenClawAccessModal({
    isOpen,
    onClose,
    showToast,
}: OpenClawAccessModalProps) {
    const {
        clients,
        loading,
        error,
        latestPrompt,
        creatingPrompt,
        activeClientMutationId,
        generateBootstrapPrompt,
        deleteClient,
        disableClient,
        enableClient,
        clearLatestPrompt,
    } = useOpenClawAccess(isOpen);

    const [label, setLabel] = useState('');
    const [expiresInMinutes, setExpiresInMinutes] = useState(15);
    const [copiedPrompt, setCopiedPrompt] = useState(false);
    const [searchQuery, setSearchQuery] = useState('');
    const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
    const [isAddPanelOpen, setIsAddPanelOpen] = useState(false);
    const [pendingAction, setPendingAction] = useState<PendingClientAction>(null);

    const sortedClients = useMemo(
        () =>
            [...clients].sort((left, right) =>
                left.enrolled_at < right.enrolled_at ? 1 : -1
            ),
        [clients]
    );

    const clientStats = useMemo(() => {
        const total = sortedClients.length;
        const active = sortedClients.filter((client) => client.status === 'active').length;
        const disabled = sortedClients.filter((client) => client.status === 'disabled').length;
        const revoked = sortedClients.filter((client) => client.status === 'revoked').length;
        const waiting = sortedClients.filter(
            (client) => client.status === 'waiting_connection'
        ).length;
        return { total, active, disabled, revoked, waiting };
    }, [sortedClients]);

    const filteredClients = useMemo(() => {
        const normalizedQuery = searchQuery.trim().toLowerCase();

        return sortedClients.filter((client) => {
            if (statusFilter !== 'all' && client.status !== statusFilter) {
                return false;
            }

            if (!normalizedQuery) return true;

            const searchableFields = [
                client.display_name,
                client.client_id,
                client.last_seen_ip ?? '',
                client.last_seen_user_agent ?? '',
                ...client.key_fingerprints,
            ];

            return searchableFields.some((field) =>
                field.toLowerCase().includes(normalizedQuery)
            );
        });
    }, [searchQuery, sortedClients, statusFilter]);

    useEffect(() => {
        if (!isOpen) {
            setPendingAction(null);
            setSearchQuery('');
            setStatusFilter('all');
            setIsAddPanelOpen(false);
        }
    }, [isOpen]);

    useEffect(() => {
        if (latestPrompt) {
            setIsAddPanelOpen(true);
        }
    }, [latestPrompt]);

    const handleLabelChange = (value: string) => {
        setLabel(value);
    };

    const handleGeneratePrompt = async () => {
        const trimmedLabel = label.trim();
        if (!trimmedLabel) {
            showToast('Bootstrap label is required.', 'error');
            return;
        }

        try {
            await generateBootstrapPrompt({
                label: trimmedLabel,
                expires_in_minutes: expiresInMinutes,
                suggested_display_name: trimmedLabel,
            });
            setCopiedPrompt(false);
            showToast('Bootstrap prompt generated successfully.', 'success');
        } catch (error) {
            showToast(
                resolveErrorMessage(error, 'Failed to generate bootstrap prompt.'),
                'error'
            );
        }
    };

    const handleResetPrompt = () => {
        clearLatestPrompt();
        setCopiedPrompt(false);
    };

    const handleDoneWithPrompt = () => {
        clearLatestPrompt();
        setCopiedPrompt(false);
        setIsAddPanelOpen(false);
    };

    const handleCopyPrompt = async () => {
        if (!latestPrompt) return;
        try {
            await navigator.clipboard.writeText(latestPrompt.prompt_text);
            setCopiedPrompt(true);
            showToast('Bootstrap prompt copied to clipboard.', 'success');
        } catch {
            showToast('Failed to copy bootstrap prompt.', 'error');
        }
    };

    const handleDisableClient = async (clientId: string) => {
        const client = sortedClients.find((entry) => entry.client_id === clientId);
        setPendingAction({
            clientId,
            clientName: client?.display_name ?? clientId,
            action: 'disable',
            title: 'Disable access?',
            message:
                'The selected installation will stop connecting until you re-enable it. Use this when you want a temporary operational block without permanently revoking the client.',
            confirmText: 'Disable Client',
            confirmVariant: 'primary',
        });
    };

    const handleEnableClient = async (clientId: string) => {
        try {
            await enableClient(clientId);
            showToast('OpenClaw client enabled.', 'success');
        } catch (error) {
            showToast(resolveErrorMessage(error, 'Failed to enable OpenClaw client.'), 'error');
        }
    };

    const handleDeleteClient = async (clientId: string) => {
        const client = sortedClients.find((entry) => entry.client_id === clientId);
        const isPendingEnrollment =
            client?.kind === 'pending' || client?.status === 'waiting_connection';
        setPendingAction({
            clientId,
            clientName: client?.display_name ?? clientId,
            action: 'delete',
            title: 'Delete installation?',
            message: isPendingEnrollment
                ? 'This will remove the waiting installation and invalidate its bootstrap token so it can no longer enroll.'
                : 'This will permanently remove the installation from ACPMS. Existing runtime traffic from this client identity will stop working.',
            confirmText: 'Delete Installation',
            confirmVariant: 'danger',
        });
    };

    const handleConfirmPendingAction = async () => {
        if (!pendingAction) return;

        try {
            if (pendingAction.action === 'disable') {
                await disableClient(pendingAction.clientId);
                showToast(`${pendingAction.clientName} disabled.`, 'success');
            } else {
                await deleteClient(pendingAction.clientId);
                showToast(`${pendingAction.clientName} deleted.`, 'success');
            }
            setPendingAction(null);
        } catch (error) {
            showToast(
                resolveErrorMessage(
                    error,
                    pendingAction.action === 'disable'
                        ? 'Failed to disable OpenClaw client.'
                        : 'Failed to delete OpenClaw installation.'
                ),
                'error'
            );
        }
    };

    return (
        <>
            <Dialog
                open={isOpen}
                onOpenChange={(open) => {
                    if (!open) onClose();
                }}
            >
                <DialogContent className="max-w-6xl max-h-[92vh] overflow-y-auto p-0">
                    <div className="flex flex-col">
                        <div className="border-b border-border bg-gradient-to-r from-primary/[0.08] via-primary/[0.03] to-transparent px-6 py-5">
                            <DialogHeader className="space-y-2">
                                <DialogTitle className="text-xl text-card-foreground">
                                    OpenClaw Access
                                </DialogTitle>
                                <DialogDescription className="text-sm text-muted-foreground">
                                    Manage additional OpenClaw installations and control access for
                                    enrolled clients.
                                </DialogDescription>
                                <p className="text-xs text-muted-foreground">
                                    The first installer-generated bootstrap prompt is handled during
                                    installation. Use this panel for later installs and ongoing access
                                    management.
                                </p>
                            </DialogHeader>
                        </div>

                        <div className="grid grid-cols-2 gap-3 border-b border-border bg-muted/20 px-6 py-4 lg:grid-cols-5">
                            {[
                                {
                                    label: 'Total installations',
                                    value: clientStats.total,
                                    hint: 'Tracked installs and pending connections',
                                },
                                {
                                    label: 'Active',
                                    value: clientStats.active,
                                    hint: 'Currently allowed to connect',
                                },
                                {
                                    label: 'Disabled',
                                    value: clientStats.disabled,
                                    hint: 'Temporarily blocked',
                                },
                                {
                                    label: 'Revoked',
                                    value: clientStats.revoked,
                                    hint: 'Permanently blocked',
                                },
                                {
                                    label: 'Waiting',
                                    value: clientStats.waiting,
                                    hint: 'Prompt issued, awaiting enrollment',
                                },
                            ].map((stat) => (
                                <div
                                    key={stat.label}
                                    className="rounded-xl border border-border bg-card px-4 py-3"
                                >
                                    <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                                        {stat.label}
                                    </p>
                                    <p className="mt-2 text-2xl font-semibold text-card-foreground">
                                        {stat.value}
                                    </p>
                                    <p className="mt-1 text-xs text-muted-foreground">{stat.hint}</p>
                                </div>
                            ))}
                        </div>

                        <div
                            data-testid="openclaw-content-grid"
                            className={`grid grid-cols-1 gap-6 p-6 transition-all duration-300 ${
                                isAddPanelOpen ? 'lg:grid-cols-3' : 'lg:grid-cols-1'
                            }`}
                        >
                            <section className="flex min-h-[420px] flex-col gap-4 rounded-xl border border-border bg-card p-5">
                                <div className="flex items-start justify-between gap-4 border-b border-border pb-3">
                                    <div>
                                        <h3 className="text-base font-semibold text-card-foreground">
                                            OpenClaw installations
                                        </h3>
                                        <p className="text-sm text-muted-foreground">
                                            View enrolled clients, track pending connections, and
                                            manage individual access.
                                        </p>
                                    </div>
                                    <div className="flex flex-wrap items-center justify-end gap-2">
                                        <span className="rounded-full border border-border bg-muted px-3 py-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                            {sortedClients.length} installation
                                            {sortedClients.length === 1 ? '' : 's'}
                                        </span>
                                        <button
                                            type="button"
                                            onClick={() =>
                                                setIsAddPanelOpen((current) => !current)
                                            }
                                            className={`rounded-lg px-3 py-2 text-xs font-semibold transition-colors ${
                                                isAddPanelOpen
                                                    ? 'border border-border bg-background text-card-foreground hover:bg-muted'
                                                    : 'bg-primary text-primary-foreground hover:bg-primary/90'
                                            }`}
                                        >
                                            {isAddPanelOpen
                                                ? 'Hide add panel'
                                                : 'Add another installation'}
                                        </button>
                                    </div>
                                </div>

                                {loading ? (
                                    <div className="rounded-lg border border-dashed border-border bg-muted/30 px-4 py-10 text-center text-sm text-muted-foreground">
                                        Loading OpenClaw clients...
                                    </div>
                                ) : error ? (
                                    <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-4 text-sm text-rose-700 dark:text-rose-300">
                                        {error}
                                    </div>
                                ) : sortedClients.length === 0 ? (
                                    <div className="rounded-xl border border-dashed border-border bg-muted/20 px-5 py-8">
                                        <p className="text-sm font-semibold text-card-foreground">
                                            No installations or pending connections yet
                                        </p>
                                        <p className="mt-2 text-sm text-muted-foreground">
                                            When you need to add another OpenClaw machine after the
                                            installer flow, open the add panel above and generate a
                                            one-time bootstrap prompt.
                                        </p>
                                        <ol className="mt-4 space-y-2 text-sm text-muted-foreground">
                                            <li>1. Create a one-time prompt for the new installation.</li>
                                            <li>2. Copy it to the target OpenClaw environment.</li>
                                            <li>3. Return here to confirm the new client appears.</li>
                                        </ol>
                                    </div>
                                ) : (
                                    <>
                                        <div className="grid gap-3 rounded-xl border border-border bg-muted/20 p-4 lg:grid-cols-[minmax(0,1fr)_220px_auto] lg:items-end">
                                            <label className="flex flex-col gap-1.5">
                                                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                                    Search installations
                                                </span>
                                                <input
                                                    aria-label="Search installations"
                                                    type="text"
                                                    value={searchQuery}
                                                    onChange={(event) =>
                                                        setSearchQuery(event.target.value)
                                                    }
                                                    placeholder="Search by name, client ID, IP, fingerprint..."
                                                    className="rounded-lg border border-border bg-background px-3 py-2.5 text-sm text-card-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                                                />
                                            </label>

                                            <label className="flex flex-col gap-1.5">
                                                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                                    Status
                                                </span>
                                                <select
                                                    aria-label="Status filter"
                                                    value={statusFilter}
                                                    onChange={(event) =>
                                                        setStatusFilter(
                                                            event.target.value as StatusFilter
                                                        )
                                                    }
                                                    className="rounded-lg border border-border bg-background px-3 py-2.5 text-sm text-card-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                                                >
                                                    <option value="all">All statuses</option>
                                                    <option value="active">Active</option>
                                                    <option value="disabled">Disabled</option>
                                                    <option value="revoked">Revoked</option>
                                                    <option value="waiting_connection">
                                                        Waiting connection
                                                    </option>
                                                </select>
                                            </label>

                                            <div className="rounded-lg border border-border bg-background px-3 py-2.5 text-xs text-muted-foreground">
                                                Showing{' '}
                                                <span className="font-semibold text-card-foreground">
                                                    {filteredClients.length}
                                                </span>{' '}
                                                of{' '}
                                                <span className="font-semibold text-card-foreground">
                                                    {sortedClients.length}
                                                </span>{' '}
                                                installations
                                            </div>
                                        </div>

                                        {filteredClients.length === 0 ? (
                                            <div className="rounded-xl border border-dashed border-border bg-muted/20 px-5 py-8">
                                                <p className="text-sm font-semibold text-card-foreground">
                                                    No installations match the current filters.
                                                </p>
                                                <p className="mt-2 text-sm text-muted-foreground">
                                                    Adjust the search query or switch the status filter
                                                    back to see more OpenClaw installations.
                                                </p>
                                            </div>
                                        ) : (
                                            <div className="flex max-h-[560px] flex-col gap-3 overflow-y-auto pr-1">
                                                {filteredClients.map((client) => {
                                                    const actionLoading =
                                                        activeClientMutationId === client.client_id;
                                                    const isPendingEnrollment =
                                                        client.kind === 'pending' ||
                                                        client.status === 'waiting_connection';
                                                    const health = isPendingEnrollment
                                                        ? {
                                                              label: 'Awaiting enrollment',
                                                              detail: 'Awaiting first enrollment',
                                                              classes:
                                                                  'border-sky-500/25 bg-sky-500/10 text-sky-700 dark:text-sky-300',
                                                          }
                                                        : getClientHealth(client.last_seen_at);

                                                    return (
                                                        <div
                                                            key={client.client_id}
                                                            className="rounded-xl border border-border bg-background/70 p-4"
                                                        >
                                                            <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
                                                                <div className="min-w-0 flex-1">
                                                                    <div className="flex flex-wrap items-center gap-2">
                                                                        <h4 className="text-sm font-semibold text-card-foreground">
                                                                            {client.display_name}
                                                                        </h4>
                                                                        <span
                                                                            className={`rounded-full border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-wide ${statusPillClasses(client.status)}`}
                                                                        >
                                                                            {formatStatusLabel(client.status)}
                                                                        </span>
                                                                        <span
                                                                            className={`rounded-full border px-2.5 py-1 text-[11px] font-semibold ${health.classes}`}
                                                                        >
                                                                            {health.label}
                                                                        </span>
                                                                    </div>
                                                                    <p className="mt-1 break-all font-mono text-[11px] text-muted-foreground">
                                                                        {isPendingEnrollment
                                                                            ? 'Awaiting first enrollment'
                                                                            : client.client_id}
                                                                    </p>
                                                                    {isPendingEnrollment ? (
                                                                        <div className="mt-3 grid grid-cols-1 gap-2 text-xs text-muted-foreground md:grid-cols-2">
                                                                            <p>
                                                                                <span className="font-semibold text-card-foreground">
                                                                                    Prompt issued:
                                                                                </span>{' '}
                                                                                {formatDateTime(client.enrolled_at)}
                                                                            </p>
                                                                            <p>
                                                                                <span className="font-semibold text-card-foreground">
                                                                                    Expires:
                                                                                </span>{' '}
                                                                                {formatDateTime(
                                                                                    client.expires_at
                                                                                )}
                                                                            </p>
                                                                            <p className="md:col-span-2">
                                                                                <span className="font-semibold text-card-foreground">
                                                                                    State:
                                                                                </span>{' '}
                                                                                {health.detail}
                                                                            </p>
                                                                        </div>
                                                                    ) : (
                                                                        <div className="mt-3 grid grid-cols-1 gap-2 text-xs text-muted-foreground md:grid-cols-2">
                                                                            <p>
                                                                                <span className="font-semibold text-card-foreground">
                                                                                    Enrolled:
                                                                                </span>{' '}
                                                                                {formatDateTime(client.enrolled_at)}
                                                                            </p>
                                                                            <p>
                                                                                <span className="font-semibold text-card-foreground">
                                                                                    Last seen:
                                                                                </span>{' '}
                                                                                {formatDateTime(client.last_seen_at)}
                                                                                <span className="ml-2 text-[11px] text-muted-foreground">
                                                                                    ({health.detail})
                                                                                </span>
                                                                            </p>
                                                                            <p className="break-all">
                                                                                <span className="font-semibold text-card-foreground">
                                                                                    Last IP:
                                                                                </span>{' '}
                                                                                {client.last_seen_ip || 'Unknown'}
                                                                            </p>
                                                                            <div>
                                                                                <span className="font-semibold text-card-foreground">
                                                                                    Key fingerprints:
                                                                                </span>{' '}
                                                                                {client.key_fingerprints.length > 0 ? (
                                                                                    <div className="mt-1 flex flex-wrap gap-1.5">
                                                                                        {client.key_fingerprints.map(
                                                                                            (fingerprint) => (
                                                                                                <span
                                                                                                    key={fingerprint}
                                                                                                    className="rounded-full border border-border bg-muted px-2 py-1 font-mono text-[11px] text-card-foreground"
                                                                                                >
                                                                                                    {compactFingerprint(
                                                                                                        fingerprint
                                                                                                    )}
                                                                                                </span>
                                                                                            )
                                                                                        )}
                                                                                    </div>
                                                                                ) : (
                                                                                    'None yet'
                                                                                )}
                                                                            </div>
                                                                        </div>
                                                                    )}
                                                                </div>

                                                                <div className="flex flex-wrap gap-2 xl:justify-end">
                                                                    {isPendingEnrollment ? null : client.status === 'active' ? (
                                                                            <button
                                                                                type="button"
                                                                                onClick={() =>
                                                                                    handleDisableClient(
                                                                                        client.client_id
                                                                                    )
                                                                                }
                                                                                disabled={actionLoading}
                                                                                className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-xs font-semibold text-amber-700 transition-colors hover:bg-amber-500/15 disabled:cursor-not-allowed disabled:opacity-50 dark:text-amber-300"
                                                                            >
                                                                                Disable Access
                                                                            </button>
                                                                        ) : client.status === 'disabled' ? (
                                                                            <button
                                                                                type="button"
                                                                                onClick={() =>
                                                                                    handleEnableClient(
                                                                                        client.client_id
                                                                                    )
                                                                                }
                                                                                disabled={actionLoading}
                                                                                className="rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-3 py-2 text-xs font-semibold text-emerald-700 transition-colors hover:bg-emerald-500/15 disabled:cursor-not-allowed disabled:opacity-50 dark:text-emerald-300"
                                                                            >
                                                                                Enable Access
                                                                            </button>
                                                                        ) : null}
                                                                    <button
                                                                        type="button"
                                                                        onClick={() =>
                                                                            handleDeleteClient(
                                                                                client.client_id
                                                                            )
                                                                        }
                                                                        disabled={actionLoading}
                                                                        className="rounded-lg border border-rose-500/25 bg-rose-500/10 px-3 py-2 text-xs font-semibold text-rose-700 transition-colors hover:bg-rose-500/15 disabled:cursor-not-allowed disabled:opacity-50 dark:text-rose-300"
                                                                    >
                                                                        Delete Installation
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        </div>
                                                    );
                                                })}
                                            </div>
                                        )}
                                    </>
                                )}
                            </section>

                            {isAddPanelOpen ? (
                                <section
                                    data-testid="openclaw-add-panel"
                                    className="flex flex-col gap-4 rounded-xl border border-border bg-card p-5 lg:col-span-2"
                                >
                                <div className="border-b border-border pb-3">
                                    <h3 className="text-base font-semibold text-card-foreground">
                                        Add another installation
                                    </h3>
                                    <p className="text-sm text-muted-foreground">
                                        Create a single-use bootstrap prompt for an additional
                                        OpenClaw machine.
                                    </p>
                                </div>

                                <div className="rounded-xl border border-border bg-muted/20 p-4">
                                    <h4 className="text-sm font-semibold text-card-foreground">
                                        Describe the new installation
                                    </h4>
                                    <p className="mt-1 text-sm text-muted-foreground">
                                        Give the install one clear label. ACPMS will reuse it in
                                        the installations list after enrollment.
                                    </p>
                                </div>

                                <div className="grid grid-cols-1 gap-4">
                                    <label htmlFor="openclaw-bootstrap-label" className="flex flex-col gap-1.5">
                                        <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                            Internal label
                                        </span>
                                        <input
                                            id="openclaw-bootstrap-label"
                                            aria-label="Internal label"
                                            type="text"
                                            value={label}
                                            onChange={(event) =>
                                                handleLabelChange(event.target.value)
                                            }
                                            placeholder="OpenClaw Staging"
                                            className="rounded-lg border border-border bg-muted px-3 py-2.5 text-sm text-card-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                                        />
                                        <span className="text-xs text-muted-foreground">
                                            Used for bootstrap token tracking, audit logs, and the
                                            final installation name after enrollment.
                                        </span>
                                    </label>

                                    <label htmlFor="openclaw-bootstrap-expiry" className="flex flex-col gap-1.5">
                                        <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                            Expires in
                                        </span>
                                        <select
                                            id="openclaw-bootstrap-expiry"
                                            aria-label="Expires in"
                                            value={expiresInMinutes}
                                            onChange={(event) =>
                                                setExpiresInMinutes(Number(event.target.value))
                                            }
                                            className="rounded-lg border border-border bg-muted px-3 py-2.5 text-sm text-card-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                                        >
                                            <option value={15}>15 minutes</option>
                                            <option value={30}>30 minutes</option>
                                            <option value={60}>1 hour</option>
                                            <option value={240}>4 hours</option>
                                        </select>
                                        <span className="text-xs text-muted-foreground">
                                            Keep this short so unused prompts expire quickly.
                                        </span>
                                    </label>
                                </div>

                                <div className="flex gap-2">
                                    <button
                                        type="button"
                                        onClick={handleGeneratePrompt}
                                        disabled={creatingPrompt}
                                        className="inline-flex items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
                                    >
                                        {creatingPrompt ? 'Generating...' : 'Generate Bootstrap Prompt'}
                                    </button>
                                </div>

                                {latestPrompt ? (
                                    <div className="rounded-xl border border-primary/15 bg-primary/[0.04] p-4">
                                        <div className="rounded-xl border border-primary/10 bg-background/70 p-4">
                                            <h4 className="text-sm font-semibold text-card-foreground">
                                                Send the prompt to the new installation
                                            </h4>
                                            <p className="mt-1 text-sm text-muted-foreground">
                                                Copy this prompt into the target OpenClaw environment.
                                                When enrollment completes, the new installation will
                                                appear on the left.
                                            </p>
                                            <div className="mt-3 grid grid-cols-1 gap-2 text-xs text-muted-foreground md:grid-cols-2">
                                                <p>
                                                    <span className="font-semibold text-card-foreground">
                                                        Token preview:
                                                    </span>{' '}
                                                    <span className="font-mono">
                                                        {latestPrompt.token_preview}
                                                    </span>
                                                </p>
                                                <p>
                                                    <span className="font-semibold text-card-foreground">
                                                        Expires:
                                                    </span>{' '}
                                                    {formatDateTime(latestPrompt.expires_at)}
                                                </p>
                                            </div>
                                        </div>
                                        <textarea
                                            readOnly
                                            value={latestPrompt.prompt_text}
                                            className="mt-3 min-h-[220px] w-full rounded-lg border border-border bg-background px-3 py-3 font-mono text-xs text-card-foreground focus:outline-none"
                                        />
                                        <div className="mt-3 flex flex-wrap gap-2">
                                            <button
                                                type="button"
                                                onClick={handleCopyPrompt}
                                                className="rounded-lg border border-border bg-background px-3 py-2 text-xs font-semibold text-card-foreground transition-colors hover:bg-muted"
                                            >
                                                {copiedPrompt ? 'Prompt Copied' : 'Copy Prompt'}
                                            </button>
                                            <button
                                                type="button"
                                                onClick={handleResetPrompt}
                                                className="rounded-lg border border-border px-3 py-2 text-xs font-semibold text-card-foreground transition-colors hover:bg-muted"
                                            >
                                                Generate Another
                                            </button>
                                            <button
                                                type="button"
                                                onClick={handleDoneWithPrompt}
                                                className="rounded-lg border border-border px-3 py-2 text-xs font-semibold text-card-foreground transition-colors hover:bg-muted"
                                            >
                                                Done
                                            </button>
                                        </div>
                                    </div>
                                ) : (
                                    <div className="rounded-xl border border-dashed border-border bg-muted/20 px-4 py-6 text-sm text-muted-foreground">
                                        Generate a prompt only when you are ready to enroll another
                                        OpenClaw installation outside the installer flow.
                                    </div>
                                )}
                                </section>
                            ) : null}
                        </div>
                    </div>
                    <ConfirmModal
                        isOpen={pendingAction !== null}
                        onClose={() => setPendingAction(null)}
                        onConfirm={handleConfirmPendingAction}
                        title={pendingAction?.title ?? 'Confirm action'}
                        message={pendingAction?.message ?? ''}
                        confirmText={pendingAction?.confirmText}
                        confirmVariant={pendingAction?.confirmVariant}
                        isLoading={
                            pendingAction !== null &&
                            activeClientMutationId === pendingAction.clientId
                        }
                    />
                </DialogContent>
            </Dialog>
        </>
    );
}
