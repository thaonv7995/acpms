/**
 * DeploymentSettingsPanel - Deployment and preview configuration
 *
 * Settings:
 * - auto_deploy: Deliver task preview when task completes - NOT production
 * - production_deploy_on_merge: Production deploy when MR is merged
 * - preview_ttl_days: Preview environment lifetime in days
 */

import { SettingRow, ToggleSwitch, NumberInput } from './settings-form-controls';
import type { ProjectSettings } from '../../../hooks/useProjectSettings';

interface DeploymentSettingsPanelProps {
    settings: ProjectSettings;
    saving: boolean;
    onUpdateSetting: <K extends keyof ProjectSettings>(key: K, value: ProjectSettings[K]) => Promise<void>;
}

export function DeploymentSettingsPanel({ settings, saving, onUpdateSetting }: DeploymentSettingsPanelProps) {
    const previewWanted = settings.auto_deploy || settings.preview_enabled;

    return (
        <div className="space-y-6">
            {/* auto_deploy = Preview when task completes (NOT production) */}
            <SettingRow
                icon="preview"
                iconColor="text-cyan-500"
                title="Task Preview khi Task xong"
                description="Web/API/Microservice publish a live preview URL. Desktop/Mobile/Extension build a downloadable test artifact. Not related to production."
                hint={
                    previewWanted
                        ? 'Each completed task will publish either a preview URL or a downloadable preview artifact for testing.'
                        : 'No preview URL or downloadable preview artifact will be created when task completes.'
                }
            >
                <ToggleSwitch
                    checked={settings.auto_deploy}
                    onChange={(checked) => onUpdateSetting('auto_deploy', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Production Deploy on Merge - Separate from preview */}
            <SettingRow
                icon="rocket_launch"
                iconColor="text-orange-500"
                title="Production Deploy khi Merge"
                description="Auto-deploy to production (Cloudflare Pages/Workers) when MR is merged into deploy branch."
                hint={
                    settings.production_deploy_on_merge
                        ? 'Production deploy will run when merged into target branch.'
                        : 'Manual deploy trigger required after merge.'
                }
            >
                <ToggleSwitch
                    checked={settings.production_deploy_on_merge ?? false}
                    onChange={(checked) => onUpdateSetting('production_deploy_on_merge', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Preview TTL Input */}
            <SettingRow
                icon="schedule"
                iconColor="text-indigo-500"
                title="Preview Lifetime"
                description="Number of days a preview environment remains active before automatic cleanup."
                hint="Shorter TTL saves resources; longer TTL allows extended testing periods."
            >
                <NumberInput
                    value={settings.preview_ttl_days}
                    onChange={(value) => onUpdateSetting('preview_ttl_days', value)}
                    min={1}
                    max={30}
                    suffix="days"
                    disabled={saving || !previewWanted}
                />
            </SettingRow>

            {/* Info Banner */}
            {!previewWanted && (
                <div className="flex items-start gap-3 p-4 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
                    <span className="material-symbols-outlined text-amber-500 text-xl">info</span>
                    <div>
                        <p className="text-sm font-medium text-amber-800 dark:text-amber-300">
                            Preview on task complete is off
                        </p>
                        <p className="text-xs text-amber-600 dark:text-amber-400 mt-1">
                            Enable to publish a task preview after completion.
                            Web/API/Microservice require Cloudflare plus PREVIEW_TARGET. Desktop/Mobile/Extension return downloadable artifacts instead.
                        </p>
                    </div>
                </div>
            )}
        </div>
    );
}
