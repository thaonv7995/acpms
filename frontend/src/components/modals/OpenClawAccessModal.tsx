import { useMemo, useState } from 'react';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from '../ui/dialog';
import { ApiError } from '../../api/client';
import { useOpenClawAccess } from '../../hooks/useOpenClawAccess';

interface OpenClawAccessModalProps {
    isOpen: boolean;
    onClose: () => void;
    showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

function formatDateTime(value: string | null): string {
    if (!value) return 'Never';
    return new Date(value).toLocaleString();
}

function statusPillClasses(status: string): string {
    switch (status) {
        case 'active':
            return 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300 border-emerald-500/25';
        case 'disabled':
            return 'bg-amber-500/15 text-amber-700 dark:text-amber-300 border-amber-500/25';
        case 'revoked':
            return 'bg-rose-500/15 text-rose-700 dark:text-rose-300 border-rose-500/25';
        default:
            return 'bg-muted text-muted-foreground border-border';
    }
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
        disableClient,
        enableClient,
        revokeClient,
        clearLatestPrompt,
    } = useOpenClawAccess(isOpen);

    const [label, setLabel] = useState('');
    const [displayName, setDisplayName] = useState('');
    const [expiresInMinutes, setExpiresInMinutes] = useState(15);
    const [copiedPrompt, setCopiedPrompt] = useState(false);

    const sortedClients = useMemo(
        () =>
            [...clients].sort((left, right) =>
                left.enrolled_at < right.enrolled_at ? 1 : -1
            ),
        [clients]
    );

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
                suggested_display_name: displayName.trim() || undefined,
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
        if (!window.confirm(`Disable access for OpenClaw client ${clientId}?`)) return;
        try {
            await disableClient(clientId);
            showToast('OpenClaw client disabled.', 'success');
        } catch (error) {
            showToast(resolveErrorMessage(error, 'Failed to disable OpenClaw client.'), 'error');
        }
    };

    const handleEnableClient = async (clientId: string) => {
        try {
            await enableClient(clientId);
            showToast('OpenClaw client enabled.', 'success');
        } catch (error) {
            showToast(resolveErrorMessage(error, 'Failed to enable OpenClaw client.'), 'error');
        }
    };

    const handleRevokeClient = async (clientId: string) => {
        if (
            !window.confirm(
                `Revoke OpenClaw client ${clientId}? This is stronger than disable and should only be used when access must be permanently blocked.`
            )
        ) {
            return;
        }

        try {
            await revokeClient(clientId);
            showToast('OpenClaw client revoked.', 'success');
        } catch (error) {
            showToast(resolveErrorMessage(error, 'Failed to revoke OpenClaw client.'), 'error');
        }
    };

    return (
        <Dialog
            open={isOpen}
            onOpenChange={(open) => {
                if (!open) onClose();
            }}
        >
            <DialogContent className="max-w-6xl p-0 overflow-hidden">
                <div className="flex flex-col">
                    <div className="border-b border-border bg-gradient-to-r from-primary/[0.08] via-primary/[0.03] to-transparent px-6 py-5">
                        <DialogHeader className="space-y-2">
                            <DialogTitle className="text-xl text-card-foreground">
                                OpenClaw Access Management
                            </DialogTitle>
                            <DialogDescription className="text-sm text-muted-foreground">
                                The installer owns the first OpenClaw bootstrap prompt. Use this
                                panel to view enrolled clients, add additional OpenClaw installs,
                                and disable, enable, or revoke individual access.
                            </DialogDescription>
                        </DialogHeader>
                    </div>

                    <div className="grid grid-cols-1 gap-6 p-6 lg:grid-cols-[1.2fr_0.8fr]">
                        <section className="flex min-h-[420px] flex-col gap-4 rounded-xl border border-border bg-card p-5">
                            <div className="flex items-center justify-between gap-4 border-b border-border pb-3">
                                <div>
                                    <h3 className="text-base font-semibold text-card-foreground">
                                        Clients
                                    </h3>
                                    <p className="text-sm text-muted-foreground">
                                        Enrolled OpenClaw installations appear here after the
                                        installer bootstrap or any later add-on enrollment.
                                    </p>
                                </div>
                                <span className="rounded-full border border-border bg-muted px-3 py-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                    {sortedClients.length} client{sortedClients.length === 1 ? '' : 's'}
                                </span>
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
                                <div className="rounded-lg border border-dashed border-border bg-muted/20 px-4 py-10 text-center">
                                    <p className="text-sm font-medium text-card-foreground">
                                        No OpenClaw clients enrolled yet.
                                    </p>
                                    <p className="mt-2 text-sm text-muted-foreground">
                                        Generate a bootstrap prompt here only when you need to add
                                        another OpenClaw installation after the installer flow.
                                    </p>
                                </div>
                            ) : (
                                <div className="flex max-h-[560px] flex-col gap-3 overflow-y-auto pr-1">
                                    {sortedClients.map((client) => {
                                        const actionLoading =
                                            activeClientMutationId === client.client_id;
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
                                                                {client.status}
                                                            </span>
                                                        </div>
                                                        <p className="mt-1 break-all font-mono text-xs text-muted-foreground">
                                                            {client.client_id}
                                                        </p>
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
                                                            </p>
                                                            <p className="break-all">
                                                                <span className="font-semibold text-card-foreground">
                                                                    Last IP:
                                                                </span>{' '}
                                                                {client.last_seen_ip || 'Unknown'}
                                                            </p>
                                                            <p className="break-all">
                                                                <span className="font-semibold text-card-foreground">
                                                                    Fingerprints:
                                                                </span>{' '}
                                                                {client.key_fingerprints.length > 0
                                                                    ? client.key_fingerprints.join(
                                                                          ', '
                                                                      )
                                                                    : 'None yet'}
                                                            </p>
                                                        </div>
                                                    </div>

                                                    <div className="flex flex-wrap gap-2 xl:justify-end">
                                                        {client.status === 'active' ? (
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
                                                                handleRevokeClient(client.client_id)
                                                            }
                                                            disabled={
                                                                actionLoading ||
                                                                client.status === 'revoked'
                                                            }
                                                            className="rounded-lg border border-rose-500/25 bg-rose-500/10 px-3 py-2 text-xs font-semibold text-rose-700 transition-colors hover:bg-rose-500/15 disabled:cursor-not-allowed disabled:opacity-50 dark:text-rose-300"
                                                        >
                                                            Revoke Client
                                                        </button>
                                                    </div>
                                                </div>
                                            </div>
                                        );
                                    })}
                                </div>
                            )}
                        </section>

                        <section className="flex flex-col gap-4 rounded-xl border border-border bg-card p-5">
                            <div className="border-b border-border pb-3">
                                <h3 className="text-base font-semibold text-card-foreground">
                                    Add OpenClaw
                                </h3>
                                <p className="text-sm text-muted-foreground">
                                    Create a single-use bootstrap prompt for an additional
                                    OpenClaw installation.
                                </p>
                            </div>

                            <div className="grid grid-cols-1 gap-4">
                                <label className="flex flex-col gap-1.5">
                                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                        Label
                                    </span>
                                    <input
                                        type="text"
                                        value={label}
                                        onChange={(event) => setLabel(event.target.value)}
                                        placeholder="OpenClaw Staging"
                                        className="rounded-lg border border-border bg-muted px-3 py-2.5 text-sm text-card-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                                    />
                                </label>

                                <label className="flex flex-col gap-1.5">
                                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                        Suggested display name
                                    </span>
                                    <input
                                        type="text"
                                        value={displayName}
                                        onChange={(event) => setDisplayName(event.target.value)}
                                        placeholder="OpenClaw Staging"
                                        className="rounded-lg border border-border bg-muted px-3 py-2.5 text-sm text-card-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                                    />
                                </label>

                                <label className="flex flex-col gap-1.5">
                                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                        Expires in
                                    </span>
                                    <select
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
                                {latestPrompt ? (
                                    <button
                                        type="button"
                                        onClick={clearLatestPrompt}
                                        className="inline-flex items-center justify-center rounded-lg border border-border px-4 py-2 text-sm font-semibold text-card-foreground transition-colors hover:bg-muted"
                                    >
                                        Clear Prompt
                                    </button>
                                ) : null}
                            </div>

                            {latestPrompt ? (
                                <div className="rounded-xl border border-primary/15 bg-primary/[0.04] p-4">
                                    <div className="flex flex-wrap items-start justify-between gap-3">
                                        <div>
                                            <p className="text-sm font-semibold text-card-foreground">
                                                Prompt Output
                                            </p>
                                            <p className="mt-1 text-xs text-muted-foreground">
                                                Token preview: {latestPrompt.token_preview}
                                            </p>
                                            <p className="mt-1 text-xs text-muted-foreground">
                                                Expires: {formatDateTime(latestPrompt.expires_at)}
                                            </p>
                                        </div>
                                        <button
                                            type="button"
                                            onClick={handleCopyPrompt}
                                            className="rounded-lg border border-border bg-background px-3 py-2 text-xs font-semibold text-card-foreground transition-colors hover:bg-muted"
                                        >
                                            {copiedPrompt ? 'Copied' : 'Copy Prompt'}
                                        </button>
                                    </div>
                                    <textarea
                                        readOnly
                                        value={latestPrompt.prompt_text}
                                        className="mt-3 min-h-[220px] w-full rounded-lg border border-border bg-background px-3 py-3 font-mono text-xs text-card-foreground focus:outline-none"
                                    />
                                </div>
                            ) : (
                                <div className="rounded-xl border border-dashed border-border bg-muted/20 px-4 py-6 text-sm text-muted-foreground">
                                    Generate a prompt here when you need a one-time bootstrap
                                    token for an additional OpenClaw installation.
                                </div>
                            )}
                        </section>
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
}
