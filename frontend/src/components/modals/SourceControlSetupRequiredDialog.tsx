import { Link } from 'react-router-dom';

interface SourceControlSetupRequiredDialogProps {
    isOpen: boolean;
    onClose: () => void;
    contextLabel: string;
}

export function SourceControlSetupRequiredDialog({
    isOpen,
    onClose,
    contextLabel,
}: SourceControlSetupRequiredDialogProps) {
    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-[70] flex items-center justify-center p-4">
            <div className="absolute inset-0 bg-black/60 backdrop-blur-[2px]" onClick={onClose} />
            <div className="relative w-full max-w-md overflow-hidden rounded-2xl border border-border bg-card shadow-2xl">
                <div className="border-b border-border px-6 py-5">
                    <div className="flex items-start gap-3">
                        <div className="mt-0.5 flex size-10 items-center justify-center rounded-lg bg-amber-500/10 text-amber-500">
                            <span className="material-symbols-outlined">settings_alert</span>
                        </div>
                        <div>
                            <h2 className="text-lg font-bold text-card-foreground">
                                Source control setup required
                            </h2>
                            <p className="mt-1 text-sm text-muted-foreground">
                                {contextLabel} is blocked until GitLab or GitHub source control is configured in System Settings.
                            </p>
                        </div>
                    </div>
                </div>

                <div className="space-y-3 px-6 py-5 text-sm text-muted-foreground">
                    <p>
                        Save your source control instance URL and Personal Access Token first.
                    </p>
                    <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-4 py-3 text-amber-700 dark:text-amber-200">
                        <p className="font-medium text-amber-800 dark:text-amber-100">Required in Settings</p>
                        <p className="mt-1 text-xs">
                            Instance URL such as <span className="font-mono">https://gitlab.com</span> or{' '}
                            <span className="font-mono">https://github.com</span>, plus a PAT with repository access.
                        </p>
                    </div>
                </div>

                <div className="flex justify-end gap-3 border-t border-border bg-muted/30 px-6 py-4">
                    <button
                        type="button"
                        onClick={onClose}
                        className="rounded-lg border border-border bg-muted px-4 py-2 text-sm font-medium text-card-foreground transition-colors hover:bg-muted/80"
                    >
                        Cancel
                    </button>
                    <Link
                        to="/settings"
                        onClick={onClose}
                        className="inline-flex items-center rounded-lg bg-primary px-4 py-2 text-sm font-bold text-primary-foreground transition-colors hover:bg-primary/90"
                    >
                        Open Settings
                    </Link>
                </div>
            </div>
        </div>
    );
}
