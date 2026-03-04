/**
 * SettingsSummary - Visual summary of current project settings configuration
 * Displays key settings at a glance in a compact footer format
 */

import type { ProjectSettings } from '../../../hooks/useProjectSettings';

interface SettingsSummaryProps {
    settings: ProjectSettings;
}

export function SettingsSummary({ settings }: SettingsSummaryProps) {
    const summaryItems = [
        {
            label: 'Review Mode',
            value: settings.require_review ? 'Required' : 'Auto-commit',
            icon: settings.require_review ? 'verified_user' : 'flash_on',
            color: settings.require_review ? 'text-green-500' : 'text-amber-500',
        },
        {
            label: 'Deployment',
            value: settings.auto_deploy ? 'Automatic' : 'Manual',
            icon: settings.auto_deploy ? 'rocket_launch' : 'touch_app',
            color: settings.auto_deploy ? 'text-orange-500' : 'text-slate-500',
        },
        {
            label: 'GitOps',
            value: settings.gitops_enabled ? 'MR Workflow' : 'Direct Push',
            icon: settings.gitops_enabled ? 'merge' : 'upload',
            color: settings.gitops_enabled ? 'text-violet-500' : 'text-red-500',
        },
        {
            label: 'Previews',
            value: settings.preview_enabled ? `${settings.preview_ttl_days}d TTL` : 'Disabled',
            icon: settings.preview_enabled ? 'preview' : 'visibility_off',
            color: settings.preview_enabled ? 'text-cyan-500' : 'text-slate-500',
        },
    ];

    return (
        <div className="bg-slate-50 dark:bg-slate-800/50 rounded-lg border border-slate-200 dark:border-slate-700 p-4">
            <div className="flex items-center gap-2 mb-3">
                <span className="material-symbols-outlined text-slate-400 text-[18px]">summarize</span>
                <span className="text-xs font-medium text-slate-500 dark:text-slate-400 uppercase tracking-wider">
                    Current Configuration
                </span>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                {summaryItems.map((item) => (
                    <div key={item.label} className="flex items-center gap-2">
                        <span className={`material-symbols-outlined text-[20px] ${item.color}`}>
                            {item.icon}
                        </span>
                        <div>
                            <p className="text-xs text-slate-500 dark:text-slate-400">{item.label}</p>
                            <p className="text-sm font-medium text-slate-900 dark:text-white">{item.value}</p>
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}
