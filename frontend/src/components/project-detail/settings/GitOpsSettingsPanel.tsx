/**
 * GitOpsSettingsPanel - Git operations and merge request configuration
 *
 * Settings:
 * - gitops_enabled: Create MRs vs direct push
 * - auto_merge: Automatically merge approved MRs
 * - deploy_branch: Target branch for deployments
 */

import { SettingRow, ToggleSwitch, TextInput } from './settings-form-controls';
import type { ProjectSettings } from '../../../hooks/useProjectSettings';
import type { RepositoryContext } from '../../../types/repository';
import {
    getRepositoryAccessSummary,
    isRepositoryReadOnly,
    normalizeRepositoryContext,
} from '../../../utils/repositoryAccess';

interface GitOpsSettingsPanelProps {
    settings: ProjectSettings;
    saving: boolean;
    onUpdateSetting: <K extends keyof ProjectSettings>(key: K, value: ProjectSettings[K]) => Promise<void>;
    repositoryContext?: RepositoryContext;
}

export function GitOpsSettingsPanel({
    settings,
    saving,
    onUpdateSetting,
    repositoryContext,
}: GitOpsSettingsPanelProps) {
    const normalizedRepositoryContext = normalizeRepositoryContext(repositoryContext);
    const repositorySummary = getRepositoryAccessSummary(normalizedRepositoryContext);
    const repositoryReadOnly = Boolean(repositoryContext) && isRepositoryReadOnly(normalizedRepositoryContext);
    const gitOpsWorkflowSupported = !repositoryReadOnly
        && normalizedRepositoryContext.can_push
        && normalizedRepositoryContext.can_open_change_request;
    const autoMergeSupported = gitOpsWorkflowSupported && normalizedRepositoryContext.can_merge;
    const gitOpsDisabledReason = repositoryContext && !gitOpsWorkflowSupported
        ? repositorySummary.action
        : undefined;
    const autoMergeDisabledReason = repositoryContext && !autoMergeSupported
        ? normalizedRepositoryContext.can_open_change_request
            ? 'Current repository access does not allow automatic merge operations.'
            : 'Current repository access does not support automatic PR/MR workflows.'
        : undefined;

    return (
        <div className="space-y-6">
            {/* GitOps Enabled Toggle */}
            <SettingRow
                icon="merge"
                iconColor="text-violet-500"
                title="GitOps Workflow"
                description="Create merge requests for agent changes instead of direct commits to the target branch."
                hint={
                    settings.gitops_enabled
                        ? 'Agent changes will create MRs for review and controlled merging.'
                        : 'Agent will commit directly to the target branch (use with caution).'
                }
            >
                <ToggleSwitch
                    checked={settings.gitops_enabled}
                    onChange={(checked) => onUpdateSetting('gitops_enabled', checked)}
                    disabled={saving || !gitOpsWorkflowSupported}
                    ariaLabel="GitOps Workflow"
                />
            </SettingRow>

            {/* Auto Merge Toggle */}
            <SettingRow
                icon="done_all"
                iconColor="text-emerald-500"
                title="Auto-Merge Approved MRs"
                description="Automatically merge merge requests once they are approved and all checks pass."
                hint={
                    settings.auto_merge
                        ? 'Approved MRs will be merged automatically without manual intervention.'
                        : 'Manual merge action required after MR approval.'
                }
            >
                <ToggleSwitch
                    checked={settings.auto_merge}
                    onChange={(checked) => onUpdateSetting('auto_merge', checked)}
                    disabled={saving || !settings.gitops_enabled || !autoMergeSupported}
                    ariaLabel="Auto-Merge Approved MRs"
                />
            </SettingRow>

            {/* Deploy Branch Input */}
            <SettingRow
                icon="account_tree"
                iconColor="text-blue-500"
                title="Deploy Branch"
                description="The target branch that triggers production deployments when changes are merged."
                hint="Common values: main, master, production, release"
            >
                <TextInput
                    value={settings.deploy_branch}
                    onChange={(value) => onUpdateSetting('deploy_branch', value)}
                    placeholder="main"
                    disabled={saving}
                />
            </SettingRow>

            {gitOpsDisabledReason && (
                <div className="flex items-start gap-3 p-4 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
                    <span className="material-symbols-outlined text-amber-500 text-xl">lock</span>
                    <div>
                        <p className="text-sm font-medium text-amber-800 dark:text-amber-300">
                            GitOps settings locked by repository access
                        </p>
                        <p className="text-xs text-amber-700 dark:text-amber-400 mt-1">
                            {gitOpsDisabledReason}
                        </p>
                    </div>
                </div>
            )}

            {!gitOpsDisabledReason && autoMergeDisabledReason && (
                <div className="flex items-start gap-3 p-4 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
                    <span className="material-symbols-outlined text-amber-500 text-xl">info</span>
                    <div>
                        <p className="text-sm font-medium text-amber-800 dark:text-amber-300">
                            Auto-merge unavailable
                        </p>
                        <p className="text-xs text-amber-700 dark:text-amber-400 mt-1">
                            {autoMergeDisabledReason}
                        </p>
                    </div>
                </div>
            )}

            {/* Warning Banner for GitOps Disabled */}
            {!settings.gitops_enabled && !gitOpsDisabledReason && (
                <div className="flex items-start gap-3 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                    <span className="material-symbols-outlined text-red-500 text-xl">warning</span>
                    <div>
                        <p className="text-sm font-medium text-red-800 dark:text-red-300">
                            GitOps workflow disabled
                        </p>
                        <p className="text-xs text-red-600 dark:text-red-400 mt-1">
                            Direct commits bypass code review processes and may introduce issues.
                            Consider enabling GitOps for safer deployments.
                        </p>
                    </div>
                </div>
            )}

            {/* Info Banner for Auto-Merge */}
            {settings.gitops_enabled && settings.auto_merge && (
                <div className="flex items-start gap-3 p-4 bg-emerald-50 dark:bg-emerald-900/20 border border-emerald-200 dark:border-emerald-800 rounded-lg">
                    <span className="material-symbols-outlined text-emerald-500 text-xl">check_circle</span>
                    <div>
                        <p className="text-sm font-medium text-emerald-800 dark:text-emerald-300">
                            Automated merge pipeline active
                        </p>
                        <p className="text-xs text-emerald-600 dark:text-emerald-400 mt-1">
                            MRs will be automatically merged after approval and passing CI checks.
                            Combined with auto-deploy for fully automated deployment pipeline.
                        </p>
                    </div>
                </div>
            )}
        </div>
    );
}
