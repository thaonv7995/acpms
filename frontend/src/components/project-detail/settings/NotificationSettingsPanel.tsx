/**
 * NotificationSettingsPanel - Notification preferences configuration
 *
 * Settings:
 * - notify_on_success: Send notification on task completion
 * - notify_on_failure: Send notification on task failure
 * - notify_on_review: Send notification when review is needed
 * - notify_channels: List of notification channels (Slack, email, etc.)
 */

import { useState } from 'react';
import { SettingRow, ToggleSwitch } from './settings-form-controls';
import type { ProjectSettings } from '../../../hooks/useProjectSettings';

interface NotificationSettingsPanelProps {
    settings: ProjectSettings;
    saving: boolean;
    onUpdateSetting: <K extends keyof ProjectSettings>(key: K, value: ProjectSettings[K]) => Promise<void>;
}

export function NotificationSettingsPanel({ settings, saving, onUpdateSetting }: NotificationSettingsPanelProps) {
    const [newChannel, setNewChannel] = useState('');

    const handleAddChannel = () => {
        if (newChannel.trim() && !settings.notify_channels.includes(newChannel.trim())) {
            onUpdateSetting('notify_channels', [...settings.notify_channels, newChannel.trim()]);
            setNewChannel('');
        }
    };

    const handleRemoveChannel = (channel: string) => {
        onUpdateSetting('notify_channels', settings.notify_channels.filter((c) => c !== channel));
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            handleAddChannel();
        }
    };

    const anyNotificationsEnabled = settings.notify_on_success || settings.notify_on_failure || settings.notify_on_review;

    return (
        <div className="space-y-6">
            {/* Notify on Success Toggle */}
            <SettingRow
                icon="check_circle"
                iconColor="text-green-500"
                title="Notify on Task Success"
                description="Send a notification when a task completes successfully."
            >
                <ToggleSwitch
                    checked={settings.notify_on_success}
                    onChange={(checked) => onUpdateSetting('notify_on_success', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Notify on Failure Toggle */}
            <SettingRow
                icon="error"
                iconColor="text-red-500"
                title="Notify on Task Failure"
                description="Send a notification when a task fails or encounters an error."
            >
                <ToggleSwitch
                    checked={settings.notify_on_failure}
                    onChange={(checked) => onUpdateSetting('notify_on_failure', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Notify on Review Toggle */}
            <SettingRow
                icon="rate_review"
                iconColor="text-amber-500"
                title="Notify on Review Required"
                description="Send a notification when agent changes are ready for human review."
            >
                <ToggleSwitch
                    checked={settings.notify_on_review}
                    onChange={(checked) => onUpdateSetting('notify_on_review', checked)}
                    disabled={saving}
                />
            </SettingRow>

            {/* Notification Channels */}
            <div className="p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg border border-slate-200 dark:border-slate-700">
                <div className="flex items-start gap-3 mb-4">
                    <div className="flex-shrink-0 w-10 h-10 rounded-lg bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-600 flex items-center justify-center">
                        <span className="material-symbols-outlined text-blue-500">campaign</span>
                    </div>
                    <div>
                        <p className="font-medium text-slate-900 dark:text-white">Notification Channels</p>
                        <p className="text-sm text-slate-500 dark:text-slate-400 mt-0.5">
                            Configure where notifications are sent (Slack webhooks, email addresses, etc.)
                        </p>
                    </div>
                </div>

                {/* Channel List */}
                {settings.notify_channels.length > 0 && (
                    <div className="flex flex-wrap gap-2 mb-4">
                        {settings.notify_channels.map((channel) => (
                            <div
                                key={channel}
                                className="flex items-center gap-1.5 px-3 py-1.5 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-600 rounded-full text-sm"
                            >
                                <span className="material-symbols-outlined text-[16px] text-slate-400">
                                    {channel.includes('@') ? 'email' : 'tag'}
                                </span>
                                <span className="text-slate-700 dark:text-slate-300 max-w-[200px] truncate">
                                    {channel}
                                </span>
                                <button
                                    onClick={() => handleRemoveChannel(channel)}
                                    disabled={saving}
                                    className="ml-1 text-slate-400 hover:text-red-500 disabled:opacity-50"
                                >
                                    <span className="material-symbols-outlined text-[16px]">close</span>
                                </button>
                            </div>
                        ))}
                    </div>
                )}

                {/* Add Channel Input */}
                <div className="flex gap-2">
                    <input
                        type="text"
                        value={newChannel}
                        onChange={(e) => setNewChannel(e.target.value)}
                        onKeyDown={handleKeyDown}
                        placeholder="Enter Slack webhook URL or email..."
                        disabled={saving || !anyNotificationsEnabled}
                        className="flex-1 px-3 py-2 text-sm rounded-lg border bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-600 text-slate-900 dark:text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary disabled:opacity-50 disabled:cursor-not-allowed"
                    />
                    <button
                        onClick={handleAddChannel}
                        disabled={saving || !newChannel.trim() || !anyNotificationsEnabled}
                        className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-medium rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1"
                    >
                        <span className="material-symbols-outlined text-[18px]">add</span>
                        Add
                    </button>
                </div>

                {/* Help Text */}
                {!anyNotificationsEnabled && (
                    <p className="text-xs text-amber-600 dark:text-amber-400 mt-3 flex items-center gap-1">
                        <span className="material-symbols-outlined text-[14px]">info</span>
                        Enable at least one notification type to configure channels.
                    </p>
                )}
            </div>
        </div>
    );
}
