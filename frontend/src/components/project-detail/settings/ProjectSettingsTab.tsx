/**
 * ProjectSettingsTab - Main tabbed container for project settings
 *
 * Organizes settings into 4 categories:
 * - Agent: Execution, review, retry settings
 * - Deployment: Auto-deploy, preview environments
 * - GitOps: MR workflow, auto-merge, deploy branch
 * - Notifications: Success/failure/review notifications
 */

import { useState } from 'react';
import { useProjectSettings } from '../../../hooks/useProjectSettings';
import type { RepositoryContext } from '../../../types/repository';
import { AgentSettingsPanel } from './AgentSettingsPanel';
import { DeploymentSettingsPanel } from './DeploymentSettingsPanel';
import { GitOpsSettingsPanel } from './GitOpsSettingsPanel';
import { NotificationSettingsPanel } from './NotificationSettingsPanel';
import { SettingsSummary } from './SettingsSummary';

interface ProjectSettingsTabProps {
    projectId: string;
    repositoryContext?: RepositoryContext;
}

type SettingsTab = 'agent' | 'deployment' | 'gitops' | 'notifications';

const TABS: { id: SettingsTab; label: string; icon: string }[] = [
    { id: 'agent', label: 'Agent', icon: 'smart_toy' },
    { id: 'deployment', label: 'Deployment', icon: 'rocket_launch' },
    { id: 'gitops', label: 'GitOps', icon: 'merge' },
    { id: 'notifications', label: 'Notifications', icon: 'notifications' },
];

export function ProjectSettingsTab({ projectId, repositoryContext }: ProjectSettingsTabProps) {
    const [activeTab, setActiveTab] = useState<SettingsTab>('agent');

    const {
        settings,
        loading,
        saving,
        error,
        isDirty,
        updateSetting,
        resetToDefaults,
    } = useProjectSettings({ projectId });

    if (loading) {
        return (
            <div className="flex items-center justify-center py-12">
                <div className="flex flex-col items-center gap-3">
                    <div className="animate-spin rounded-full h-10 w-10 border-b-2 border-primary"></div>
                    <p className="text-sm text-slate-500 dark:text-slate-400">Loading settings...</p>
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="flex items-center justify-center py-12">
                <div className="flex flex-col items-center gap-3 text-center">
                    <span className="material-symbols-outlined text-4xl text-red-500">error</span>
                    <p className="text-slate-700 dark:text-slate-300 font-medium">Failed to load settings</p>
                    <p className="text-sm text-slate-500 dark:text-slate-400">{error}</p>
                </div>
            </div>
        );
    }

    if (!settings) {
        return (
            <div className="flex items-center justify-center py-12">
                <p className="text-slate-500 dark:text-slate-400">No settings available</p>
            </div>
        );
    }

    return (
        <div className="space-y-6">
            {/* Header with Reset Button */}
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-xl font-bold text-slate-900 dark:text-white flex items-center gap-2">
                        <span className="material-symbols-outlined text-primary">tune</span>
                        Project Settings
                    </h2>
                    <p className="text-sm text-slate-500 dark:text-slate-400 mt-1">
                        Configure agent behavior, deployments, and notifications for this project.
                    </p>
                </div>
                <div className="flex items-center gap-3">
                    {saving && (
                        <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-slate-400">
                            <span className="material-symbols-outlined text-[18px] animate-spin">progress_activity</span>
                            Saving...
                        </div>
                    )}
                    {isDirty && !saving && (
                        <div className="flex items-center gap-1.5 px-2 py-1 bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400 rounded text-xs font-medium">
                            <span className="material-symbols-outlined text-[14px]">edit</span>
                            Modified
                        </div>
                    )}
                    <button
                        onClick={resetToDefaults}
                        disabled={saving || !isDirty}
                        className="px-3 py-1.5 text-sm font-medium text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5"
                    >
                        <span className="material-symbols-outlined text-[18px]">restart_alt</span>
                        Reset to Defaults
                    </button>
                </div>
            </div>

            {/* Tab Navigation */}
            <div className="border-b border-slate-200 dark:border-slate-700">
                <nav className="flex gap-1" aria-label="Settings tabs">
                    {TABS.map((tab) => {
                        const isActive = activeTab === tab.id;
                        return (
                            <button
                                key={tab.id}
                                onClick={() => setActiveTab(tab.id)}
                                className={`
                                    flex items-center gap-2 px-4 py-2.5 font-medium text-sm rounded-t-lg transition-colors
                                    ${isActive
                                        ? 'text-primary border-b-2 border-primary bg-primary/5'
                                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white hover:bg-slate-50 dark:hover:bg-slate-800'
                                    }
                                `}
                                role="tab"
                                aria-selected={isActive}
                                aria-controls={`tabpanel-${tab.id}`}
                            >
                                <span className="material-symbols-outlined text-base">{tab.icon}</span>
                                {tab.label}
                            </button>
                        );
                    })}
                </nav>
            </div>

            {/* Tab Content */}
            <div className="bg-white dark:bg-surface-dark border border-slate-200 dark:border-slate-700 rounded-xl p-6">
                {activeTab === 'agent' && (
                    <AgentSettingsPanel settings={settings} saving={saving} onUpdateSetting={updateSetting} />
                )}
                {activeTab === 'deployment' && (
                    <DeploymentSettingsPanel settings={settings} saving={saving} onUpdateSetting={updateSetting} />
                )}
                {activeTab === 'gitops' && (
                    <GitOpsSettingsPanel
                        settings={settings}
                        saving={saving}
                        onUpdateSetting={updateSetting}
                        repositoryContext={repositoryContext}
                    />
                )}
                {activeTab === 'notifications' && (
                    <NotificationSettingsPanel settings={settings} saving={saving} onUpdateSetting={updateSetting} />
                )}
            </div>

            {/* Settings Summary Footer */}
            <SettingsSummary settings={settings} />
        </div>
    );
}
