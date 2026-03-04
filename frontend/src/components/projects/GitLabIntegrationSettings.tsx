import { useState, FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { ApiError } from '../../api/client';
import { linkGitLabProject } from '../../api/gitlab';
import { useSettings } from '../../hooks/useSettings';

interface GitLabIntegrationSettingsProps {
    projectId: string;
    onClose: () => void;
    onSuccess: () => void;
}

export function GitLabIntegrationSettings({ projectId, onClose, onSuccess }: GitLabIntegrationSettingsProps) {
    const [repositoryUrl, setRepositoryUrl] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState('');

    const { settings, loading: settingsLoading } = useSettings();
    const gitlabConfigured = settings?.gitlab?.configured ?? false;

    const handleSubmit = async (e: FormEvent) => {
        e.preventDefault();
        setLoading(true);
        setError('');

        try {
            const url = repositoryUrl.trim();
            if (!url) {
                setError('Please paste repository URL (e.g. https://gitlab.com/group/repo)');
                setLoading(false);
                return;
            }
            await linkGitLabProject(projectId, { repository_url: url });
            onSuccess();
            onClose();
        } catch (err) {
            if (err instanceof ApiError) {
                setError(err.message);
            } else {
                setError('Failed to link GitLab project');
            }
        } finally {
            setLoading(false);
        }
    };

    if (settingsLoading) {
        return (
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
                <div className="absolute inset-0 bg-black/50 backdrop-blur-[2px]" onClick={onClose} />
                <div className="relative bg-card border border-border rounded-xl shadow-2xl w-full max-w-md p-6">
                    <div className="animate-pulse text-center text-muted-foreground">Loading...</div>
                </div>
            </div>
        );
    }

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
            <div className="absolute inset-0 bg-black/50 backdrop-blur-[2px]" onClick={onClose} />
            <div className="relative bg-card border border-border rounded-xl shadow-2xl w-full max-w-md overflow-hidden">
                {/* Header */}
                <div className="flex items-center justify-between px-6 py-5 border-b border-border shrink-0">
                    <div className="flex items-center gap-3">
                        <div className="size-10 rounded-lg bg-primary/10 flex items-center justify-center">
                            <span className="material-symbols-outlined text-primary">merge</span>
                        </div>
                        <div>
                            <h2 className="text-lg font-bold text-card-foreground">Link GitLab Project</h2>
                            <p className="text-xs text-muted-foreground mt-0.5">Connect GitLab repository to project</p>
                        </div>
                    </div>
                    <button
                        onClick={onClose}
                        className="text-muted-foreground hover:text-card-foreground transition-colors p-1 hover:bg-muted rounded-lg"
                    >
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {!gitlabConfigured ? (
                    <div className="px-6 py-5 space-y-4">
                        <div className="p-4 bg-warning/10 border border-warning/30 rounded-lg text-sm">
                            <p className="font-medium text-card-foreground">GitLab is not configured</p>
                            <p className="mt-1 text-muted-foreground">
                                Please configure GitLab URL and Personal Access Token in System Settings first.
                            </p>
                        </div>

                        <div className="flex justify-end gap-3 pt-4 border-t border-border">
                            <button
                                type="button"
                                onClick={onClose}
                                className="px-4 py-2 bg-muted hover:bg-muted/80 text-card-foreground border border-border rounded-lg text-sm font-medium transition-colors"
                            >
                                Cancel
                            </button>
                            <Link
                                to="/settings"
                                className="px-4 py-2 bg-primary text-primary-foreground hover:bg-primary/90 rounded-lg text-sm font-medium inline-flex items-center transition-colors"
                            >
                                Go to System Settings
                            </Link>
                        </div>
                    </div>
                ) : (
                    <form onSubmit={handleSubmit} className="flex flex-col">
                        <div className="px-6 py-5 space-y-4">
                            {error && (
                                <div className="p-3 bg-destructive/10 border border-destructive/30 rounded-lg text-destructive text-sm flex items-start gap-2">
                                    <span className="material-symbols-outlined text-[18px] shrink-0">error</span>
                                    <span className="flex-1">{error}</span>
                                </div>
                            )}

                            <div className="p-4 bg-muted/50 border border-border rounded-lg">
                                <p className="text-sm text-card-foreground">
                                    Using GitLab at <strong>{settings?.gitlab?.url}</strong>
                                </p>
                                <p className="text-xs mt-1">
                                    <Link to="/settings" className="text-primary hover:underline">Change in System Settings</Link>
                                </p>
                            </div>

                            <div>
                                <label htmlFor="repoUrl" className="block text-sm font-medium text-card-foreground mb-1.5">
                                    Repository URL
                                </label>
                                <input
                                    type="url"
                                    id="repoUrl"
                                    value={repositoryUrl}
                                    onChange={(e) => setRepositoryUrl(e.target.value)}
                                    required
                                    className="w-full px-3 py-2 bg-muted border border-border text-card-foreground rounded-lg text-sm focus:ring-1 focus:ring-primary focus:border-primary placeholder:text-muted-foreground transition-colors"
                                    placeholder="https://gitlab.com/group/repo or git@gitlab.com:group/repo"
                                />
                                <p className="mt-1 text-xs text-muted-foreground">
                                    Paste repository URL from GitLab (HTTPS or SSH)
                                </p>
                            </div>
                        </div>

                        <div className="flex justify-end gap-3 px-6 py-4 border-t border-border bg-muted/30">
                            <button
                                type="button"
                                onClick={onClose}
                                className="px-4 py-2 bg-muted hover:bg-muted/80 text-card-foreground border border-border rounded-lg text-sm font-medium transition-colors"
                            >
                                Cancel
                            </button>
                            <button
                                type="submit"
                                disabled={loading || !repositoryUrl.trim()}
                                className="px-4 py-2 bg-primary text-primary-foreground hover:bg-primary/90 rounded-lg text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                            >
                                {loading ? 'Linking...' : 'Link Project'}
                            </button>
                        </div>
                    </form>
                )}
            </div>
        </div>
    );
}
