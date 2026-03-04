import type { ImportProjectPreflightResponse } from '../../../types/repository';
import {
    getRepositoryAccessSummary,
    getRepositoryAccessTone,
    getRepositoryModeLabel,
    getRepositoryProviderLabel,
    getRepositoryVerificationLabel,
    normalizeRepositoryContext,
} from '../../../utils/repositoryAccess';

interface GitLabImportFormProps {
    projectName: string;
    repoUrl: string;
    upstreamRepoUrl: string;
    preflight: ImportProjectPreflightResponse | null;
    preflightLoading: boolean;
    preflightError: string | null;
    forkPending: boolean;
    onProjectNameChange: (name: string) => void;
    onRepoUrlChange: (url: string) => void;
    onCreateFork: () => void;
}

export function GitLabImportForm({
    projectName,
    repoUrl,
    upstreamRepoUrl,
    preflight,
    preflightLoading,
    preflightError,
    forkPending,
    onProjectNameChange,
    onRepoUrlChange,
    onCreateFork
}: GitLabImportFormProps) {
    const repositoryContext = normalizeRepositoryContext(preflight?.repository_context);
    const accessSummary = getRepositoryAccessSummary(repositoryContext);
    const accessTone = getRepositoryAccessTone(repositoryContext);
    const toneClasses = accessTone === 'success'
        ? 'bg-emerald-50 border-emerald-200 text-emerald-900 dark:bg-emerald-500/15 dark:border-emerald-500/30 dark:text-emerald-100'
        : accessTone === 'warning'
            ? 'bg-amber-50 border-amber-200 text-amber-900 dark:bg-amber-500/15 dark:border-amber-500/30 dark:text-amber-100'
            : 'bg-slate-50 border-slate-200 text-slate-900 dark:bg-slate-500/15 dark:border-slate-500/30 dark:text-slate-100';

    const capabilityPills = [
        { label: 'Clone', enabled: Boolean(repositoryContext.can_clone) },
        { label: 'Push', enabled: Boolean(repositoryContext.can_push) },
        { label: 'PR/MR', enabled: Boolean(repositoryContext.can_open_change_request) },
        { label: 'Merge', enabled: Boolean(repositoryContext.can_merge) },
    ];
    const showAutoForkAction =
        Boolean(preflight) &&
        !upstreamRepoUrl &&
        Boolean(repositoryContext.can_fork) &&
        (!repositoryContext.can_push || !repositoryContext.can_open_change_request);

    return (
        <div className="flex flex-col gap-6">
            <div className="bg-blue-50 dark:bg-blue-500/20 border border-blue-100 dark:border-blue-500/30 p-4 rounded-lg flex items-start gap-3">
                <span className="material-symbols-outlined text-blue-600 dark:text-blue-400 mt-0.5">info</span>
                <p className="text-sm text-blue-800 dark:text-blue-200">
                    ACPMS can import public repositories for analysis, but writable access is required if you want the
                    agent to push code and open PRs or MRs.
                </p>
            </div>

            <div className="flex items-center gap-3 p-3 rounded-lg bg-muted border border-border">
                <div className="size-8 rounded bg-[#FC6D26]/10 flex items-center justify-center flex-shrink-0">
                    <span className="material-symbols-outlined text-[#FC6D26] text-xl">code</span>
                </div>
                <div className="flex-1">
                    <p className="text-sm font-bold text-card-foreground">GitLab or GitHub</p>
                    <p className="text-xs text-muted-foreground">Using configured instance (Settings)</p>
                </div>
                <span className="material-symbols-outlined text-green-500">check_circle</span>
            </div>

            <div className="space-y-4">
                <div>
                    <label className="block text-sm font-bold text-card-foreground mb-1.5">Project Name</label>
                    <input
                        type="text"
                        value={projectName}
                        onChange={(e) => onProjectNameChange(e.target.value)}
                        placeholder="My Project"
                        className="w-full bg-muted border border-border rounded-lg py-2.5 px-4 text-card-foreground focus:ring-primary focus:border-primary"
                    />
                </div>
                <div>
                    <label className="block text-sm font-bold text-card-foreground mb-1.5">
                        {upstreamRepoUrl ? 'Writable fork URL' : 'Repository URL'}
                    </label>
                    <div className="relative">
                        <span className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground material-symbols-outlined text-[20px]">link</span>
                        <input
                            type="text"
                            value={repoUrl}
                            onChange={(e) => onRepoUrlChange(e.target.value)}
                            placeholder="https://gitlab.com/username/project.git or https://github.com/owner/repo"
                            className="w-full bg-muted border border-border rounded-lg py-2.5 pl-10 pr-4 text-card-foreground focus:ring-primary focus:border-primary"
                        />
                    </div>
                </div>
                {upstreamRepoUrl && (
                    <div className="rounded-lg border border-border bg-muted/50 p-4 space-y-2">
                        <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                            Upstream repository
                        </p>
                        <p className="text-sm text-card-foreground break-all">{upstreamRepoUrl}</p>
                        <p className="text-xs text-muted-foreground">
                            ACPMS will clone and push via the writable fork above, then open PRs or MRs back to this upstream repository.
                        </p>
                    </div>
                )}
            </div>

            {repoUrl.trim().length > 0 && (
                <div className="space-y-3">
                    {preflightLoading && (
                        <div className="rounded-lg border border-border bg-muted/60 px-4 py-3 flex items-center gap-3">
                            <span className="inline-block size-4 rounded-full border-2 border-primary/30 border-t-primary animate-spin" />
                            <div>
                                <p className="text-sm font-medium text-card-foreground">Checking repository access</p>
                                <p className="text-xs text-muted-foreground">Validating cloneability and provider capabilities.</p>
                            </div>
                        </div>
                    )}

                    {preflightError && !preflightLoading && (
                        <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 dark:border-red-500/30 dark:bg-red-500/15">
                            <div className="flex items-start gap-3">
                                <span className="material-symbols-outlined text-red-600 dark:text-red-300">error</span>
                                <div>
                                    <p className="text-sm font-semibold text-red-900 dark:text-red-100">Repository preflight failed</p>
                                    <p className="text-xs text-red-700 dark:text-red-200 mt-1">{preflightError}</p>
                                </div>
                            </div>
                        </div>
                    )}

                    {preflight && !preflightLoading && !preflightError && (
                        <div className={`rounded-xl border px-4 py-4 space-y-4 ${toneClasses}`}>
                            <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                                <div>
                                    <p className="text-sm font-semibold">{accessSummary.title}</p>
                                    <p className="text-xs mt-1 opacity-90">{accessSummary.description}</p>
                                </div>
                                <div className="flex flex-wrap gap-2">
                                    <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold bg-white/70 text-current dark:bg-black/20">
                                        {getRepositoryProviderLabel(repositoryContext.provider)}
                                    </span>
                                    <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold bg-white/70 text-current dark:bg-black/20">
                                        {getRepositoryModeLabel(repositoryContext.access_mode)}
                                    </span>
                                    <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold bg-white/70 text-current dark:bg-black/20">
                                        {getRepositoryVerificationLabel(repositoryContext.verification_status)}
                                    </span>
                                </div>
                            </div>

                            <div className="flex flex-wrap gap-2">
                                {capabilityPills.map((capability) => (
                                    <span
                                        key={capability.label}
                                        className={`px-2.5 py-1 rounded-full text-[11px] font-semibold border ${
                                            capability.enabled
                                                ? 'border-emerald-500/40 bg-emerald-500/10'
                                                : 'border-current/15 bg-white/50 dark:bg-black/10'
                                        }`}
                                    >
                                        {capability.enabled ? 'Yes' : 'No'} {capability.label}
                                    </span>
                                ))}
                            </div>

                            <div className="space-y-2">
                                <p className="text-xs font-semibold uppercase tracking-[0.16em] opacity-70">
                                    Recommended action
                                </p>
                                <p className="text-sm">{preflight.recommended_action || accessSummary.action}</p>
                            </div>

                            {showAutoForkAction && (
                                <div className="rounded-lg border border-current/15 bg-white/60 dark:bg-black/10 px-4 py-3 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                                    <div>
                                        <p className="text-sm font-semibold">Need a writable fork?</p>
                                        <p className="text-xs mt-1 opacity-90">
                                            ACPMS can create a fork automatically, then import the project in fork-based GitOps mode.
                                        </p>
                                    </div>
                                    <button
                                        type="button"
                                        onClick={onCreateFork}
                                        disabled={forkPending}
                                        className="px-4 py-2 rounded-lg text-sm font-semibold bg-card text-card-foreground border border-border hover:bg-muted transition-colors disabled:opacity-60 flex items-center justify-center gap-2"
                                    >
                                        {forkPending ? (
                                            <>
                                                <span className="inline-block size-4 rounded-full border-2 border-current/25 border-t-current animate-spin" />
                                                Creating fork...
                                            </>
                                        ) : (
                                            <>
                                                <span className="material-symbols-outlined text-[18px]">fork_right</span>
                                                Create fork automatically
                                            </>
                                        )}
                                    </button>
                                </div>
                            )}

                            {preflight.warnings.length > 0 && (
                                <div className="space-y-2">
                                    <p className="text-xs font-semibold uppercase tracking-[0.16em] opacity-70">
                                        Warnings
                                    </p>
                                    <ul className="space-y-1">
                                        {preflight.warnings.map((warning) => (
                                            <li key={warning} className="text-sm flex items-start gap-2">
                                                <span className="material-symbols-outlined text-[16px] mt-0.5">subdirectory_arrow_right</span>
                                                <span>{warning}</span>
                                            </li>
                                        ))}
                                    </ul>
                                </div>
                            )}
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
