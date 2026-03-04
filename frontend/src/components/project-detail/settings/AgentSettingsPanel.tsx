/**
 * AgentSettingsPanel - Agent execution configuration
 *
 * Settings:
 * - require_review: Human review before commit
 * - auto_execute: Auto-start task execution on creation
 * - auto_execute_types: Task types to auto-execute
 * - auto_execute_priority: Queue priority for auto-execute
 * - auto_retry: Auto-retry failed tasks
 * - retry_backoff: Retry backoff strategy
 * - max_retries: Maximum retry attempts for failed tasks
 * - max_concurrent: Maximum concurrent tasks
 * - timeout_mins: Agent execution timeout in minutes
 */

import { SettingRow, ToggleSwitch, NumberInput, SelectInput, MultiSelectChips } from './settings-form-controls';
import type { ProjectSettings } from '../../../hooks/useProjectSettings';

const TASK_TYPE_OPTIONS = [
    { value: 'bug', label: 'Bug' },
    { value: 'hotfix', label: 'Hotfix' },
    { value: 'feature', label: 'Feature' },
    { value: 'refactor', label: 'Refactor' },
    { value: 'docs', label: 'Docs' },
    { value: 'test', label: 'Test' },
    { value: 'chore', label: 'Chore' },
    { value: 'spike', label: 'Spike' },
    { value: 'small_task', label: 'Small Task' },
    { value: 'deploy', label: 'Deploy' },
];

const PRIORITY_OPTIONS = [
    { value: 'low', label: 'Low' },
    { value: 'normal', label: 'Normal' },
    { value: 'high', label: 'High' },
];

const BACKOFF_OPTIONS = [
    { value: 'fixed', label: 'Fixed' },
    { value: 'exponential', label: 'Exponential' },
];

interface AgentSettingsPanelProps {
    settings: ProjectSettings;
    saving: boolean;
    onUpdateSetting: <K extends keyof ProjectSettings>(key: K, value: ProjectSettings[K]) => Promise<void>;
}

export function AgentSettingsPanel({ settings, saving, onUpdateSetting }: AgentSettingsPanelProps) {
    return (
        <div className="space-y-6">
            {/* Require Review Toggle */}
            <SettingRow
                icon="verified_user"
                iconColor="text-green-500"
                title="Require Human Review"
                description="Agent changes must be reviewed and approved before being committed to the repository."
                hint={
                    settings.require_review
                        ? 'Review required: Agent implements changes but does NOT commit. You review diffs and approve before pushing.'
                        : 'Auto-commit: Agent implements AND commits changes directly to the repository.'
                }
            >
                <ToggleSwitch
                    checked={settings.require_review}
                    onChange={(checked) => onUpdateSetting('require_review', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Auto Execute Toggle */}
            <SettingRow
                icon="play_circle"
                iconColor="text-blue-500"
                title="Auto-Execute Tasks"
                description="Automatically start task execution when a new task is created."
                hint={
                    settings.auto_execute
                        ? 'Tasks will automatically be assigned to an agent and start executing.'
                        : 'Tasks will be created in pending state and require manual execution start.'
                }
            >
                <ToggleSwitch
                    checked={settings.auto_execute}
                    onChange={(checked) => onUpdateSetting('auto_execute', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Auto Execute Types - only shown when auto_execute is enabled */}
            {settings.auto_execute && (
                <SettingRow
                    icon="category"
                    iconColor="text-blue-400"
                    title="Auto-Execute Task Types"
                    description="Select which task types should be auto-executed. Leave empty to auto-execute all types."
                >
                    <MultiSelectChips
                        selected={settings.auto_execute_types}
                        onChange={(types) => onUpdateSetting('auto_execute_types', types)}
                        options={TASK_TYPE_OPTIONS}
                        disabled={saving}
                    />
                </SettingRow>
            )}

            {/* Auto Execute Priority - only shown when auto_execute is enabled */}
            {settings.auto_execute && (
                <SettingRow
                    icon="priority_high"
                    iconColor="text-orange-500"
                    title="Auto-Execute Priority"
                    description="Queue priority for auto-executed tasks."
                >
                    <SelectInput
                        value={settings.auto_execute_priority}
                        onChange={(value) => onUpdateSetting('auto_execute_priority', value as 'low' | 'normal' | 'high')}
                        options={PRIORITY_OPTIONS}
                        disabled={saving}
                    />
                </SettingRow>
            )}

            {/* Auto Retry Toggle */}
            <SettingRow
                icon="autorenew"
                iconColor="text-teal-500"
                title="Auto-Retry Failed Tasks"
                description="Automatically retry tasks that fail during execution."
                hint={
                    settings.auto_retry
                        ? 'Failed tasks will be automatically retried up to the max retry limit.'
                        : 'Failed tasks will remain in failed state until manually retried.'
                }
            >
                <ToggleSwitch
                    checked={settings.auto_retry}
                    onChange={(checked) => onUpdateSetting('auto_retry', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Retry Backoff - only shown when auto_retry is enabled */}
            {settings.auto_retry && (
                <SettingRow
                    icon="schedule"
                    iconColor="text-teal-400"
                    title="Retry Backoff Strategy"
                    description="How to space out retry attempts. Exponential doubles the wait time between retries."
                >
                    <SelectInput
                        value={settings.retry_backoff}
                        onChange={(value) => onUpdateSetting('retry_backoff', value as 'fixed' | 'exponential')}
                        options={BACKOFF_OPTIONS}
                        disabled={saving}
                    />
                </SettingRow>
            )}

            {/* Max Retries Input */}
            <SettingRow
                icon="refresh"
                iconColor="text-amber-500"
                title="Max Retry Attempts"
                description="Maximum number of times to automatically retry a failed task execution."
            >
                <NumberInput
                    value={settings.max_retries}
                    onChange={(value) => onUpdateSetting('max_retries', value)}
                    min={0}
                    max={10}
                    disabled={saving}
                />
            </SettingRow>

            {/* Max Concurrent Input */}
            <SettingRow
                icon="layers"
                iconColor="text-indigo-500"
                title="Max Concurrent Tasks"
                description="Maximum number of tasks that can run simultaneously for this project."
                hint="Higher values may increase resource usage but speed up overall execution."
            >
                <NumberInput
                    value={settings.max_concurrent}
                    onChange={(value) => onUpdateSetting('max_concurrent', value)}
                    min={1}
                    max={10}
                    disabled={saving}
                />
            </SettingRow>

            {/* Timeout Input */}
            <SettingRow
                icon="timer"
                iconColor="text-purple-500"
                title="Execution Timeout"
                description="Maximum time in minutes for agent execution before timing out."
                hint="Recommended: 30 minutes for standard tasks, up to 60 for complex operations."
            >
                <NumberInput
                    value={settings.timeout_mins}
                    onChange={(value) => onUpdateSetting('timeout_mins', value)}
                    min={5}
                    max={120}
                    suffix="min"
                    disabled={saving}
                />
            </SettingRow>
        </div>
    );
}
