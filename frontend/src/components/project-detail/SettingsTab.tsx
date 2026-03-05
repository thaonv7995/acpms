// SettingsTab Component for ProjectDetail
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { GitLabIntegrationSettings } from '../projects/GitLabIntegrationSettings';
import { ProjectMembersPanel } from './ProjectMembersPanel';
import { deleteProject, syncProjectRepository } from '../../api/projects';
import { useUpdateProject } from '../../api/generated/projects/projects';
import { useProjectMembers } from '../../hooks/useProjectMembers';
import { getCurrentUser, isSystemAdmin } from '../../api/auth';
import { logger } from '@/lib/logger';

interface SettingsTabProps {
    projectId: string;
    projectName: string;
    repositoryUrl?: string;
    requireReview: boolean;
    onRefresh: () => void;
}

export function SettingsTab({ projectId, projectName, repositoryUrl, requireReview, onRefresh }: SettingsTabProps) {
    const { members, setMembers, loading: membersLoading } = useProjectMembers(projectId);
    const currentUser = getCurrentUser();
    const hasRepositoryLink = !!repositoryUrl?.trim();
    const canLinkGitLab = isSystemAdmin(currentUser);
    const canManageMembers = currentUser && members.some(
        (m) => m.id === currentUser.id && m.roles.includes('owner')
    );
    // Delete project: Owner or Admin only (ManageProject permission)
    const canDeleteProject = isSystemAdmin(currentUser) || (currentUser && members.some((m) => {
        if (m.id !== currentUser!.id) return false;
        const roles = m.roles.map((r) => r.toLowerCase());
        return roles.includes('owner') || roles.includes('admin');
    }));
    const navigate = useNavigate();
    const [showGitLabModal, setShowGitLabModal] = useState(false);
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const [deleteConfirmText, setDeleteConfirmText] = useState('');
    const [deleteLocalFolder, setDeleteLocalFolder] = useState(false);
    const [deleteGitRepo, setDeleteGitRepo] = useState(false);
    const [deleting, setDeleting] = useState(false);
    const [deleteError, setDeleteError] = useState('');
    const [syncing, setSyncing] = useState(false);

    // Update project mutation
    const updateProjectMutation = useUpdateProject();

    const handleToggleRequireReview = async () => {
        try {
            await updateProjectMutation.mutateAsync({
                id: projectId,
                data: { require_review: !requireReview }
            });
            onRefresh();
        } catch (err) {
            logger.error('Failed to update require_review:', err);
        }
    };

    const handleDelete = async () => {
        if (deleteConfirmText !== projectName) return;

        setDeleting(true);
        setDeleteError('');
        try {
            await deleteProject(projectId, {
                deleteLocalFolder,
                deleteGitRepo,
            });
            navigate('/projects');
        } catch (err) {
            setDeleteError(err instanceof Error ? err.message : 'Failed to delete project');
            setDeleting(false);
        }
    };

    return (
        <div className="space-y-6">
            {/* General Settings */}
            <div className="bg-card border border-border rounded-xl p-6">
                <h3 className="text-lg font-bold text-card-foreground mb-4 flex items-center gap-2">
                    <span className="material-symbols-outlined text-primary">settings</span>
                    General Settings
                </h3>
                <div className="space-y-4">
                    <div>
                        <label className="block text-sm font-medium text-card-foreground mb-1">
                            Project Name
                        </label>
                        <input
                            type="text"
                            value={projectName}
                            disabled
                            className="w-full px-4 py-2 bg-muted border border-border rounded-lg text-card-foreground disabled:opacity-60"
                        />
                        <p className="text-xs text-muted-foreground mt-1">Project name cannot be changed after creation</p>
                    </div>
                </div>
            </div>

            {/* Agent Settings */}
            <div className="bg-card border border-border rounded-xl p-6">
                <h3 className="text-lg font-bold text-card-foreground mb-4 flex items-center gap-2">
                    <span className="material-symbols-outlined text-purple-500">smart_toy</span>
                    Agent Settings
                </h3>
                <div className="space-y-4">
                    <div className="flex items-center justify-between p-4 bg-muted/50 rounded-lg">
                        <div className="flex-1">
                            <p className="font-medium text-card-foreground">Require Human Review</p>
                            <p className="text-sm text-muted-foreground mt-1">
                                When enabled, agent changes must be reviewed and approved before being committed to the repository.
                            </p>
                        </div>
                        <button
                            onClick={handleToggleRequireReview}
                            disabled={updateProjectMutation.isPending}
                            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 ${
                                requireReview ? 'bg-primary' : 'bg-muted'
                            } ${updateProjectMutation.isPending ? 'opacity-50 cursor-not-allowed' : ''}`}
                        >
                            <span
                                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                                    requireReview ? 'translate-x-6' : 'translate-x-1'
                                }`}
                            />
                        </button>
                    </div>
                    <p className="text-xs text-muted-foreground">
                        {requireReview
                            ? '✅ Review required: Agent will implement changes but NOT commit. You review diffs and approve before pushing.'
                            : '⚡ Auto-commit: Agent will implement AND commit changes directly to the repository.'}
                    </p>
                </div>
            </div>

            {/* GitLab Integration */}
            <div className="bg-card border border-border rounded-xl p-6">
                <h3 className="text-lg font-bold text-card-foreground mb-4 flex items-center gap-2">
                    <span className="material-symbols-outlined text-orange-500">merge</span>
                    GitLab Integration
                </h3>
                {repositoryUrl ? (
                    <div className="space-y-4">
                        <div className="flex items-center gap-3 p-4 bg-green-50 dark:bg-green-500/20 border border-green-200 dark:border-green-500/30 rounded-lg">
                            <span className="material-symbols-outlined text-green-600 dark:text-green-400">check_circle</span>
                            <div>
                                <p className="text-sm font-medium text-green-800 dark:text-green-300">Connected to GitLab</p>
                                <p className="text-xs text-green-600 dark:text-green-400">{repositoryUrl}</p>
                            </div>
                        </div>
                        <div className="flex gap-2">
                            <button
                                onClick={async () => {
                                    setSyncing(true);
                                    try {
                                        await syncProjectRepository(projectId);
                                        onRefresh();
                                    } catch (err) {
                                        logger.error('Sync failed:', err);
                                    } finally {
                                        setSyncing(false);
                                    }
                                }}
                                disabled={syncing}
                                className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-medium rounded-lg transition-colors disabled:opacity-50 flex items-center gap-2"
                            >
                                <span className={`material-symbols-outlined text-[18px] ${syncing ? 'animate-spin' : ''}`}>
                                    sync
                                </span>
                                {syncing ? 'Syncing...' : 'Sync with Git'}
                            </button>
                            {canLinkGitLab && (
                                <button
                                    onClick={() => setShowGitLabModal(true)}
                                    className="px-4 py-2 bg-muted hover:bg-muted/80 text-card-foreground text-sm font-medium rounded-lg transition-colors"
                                >
                                    Change GitLab Project
                                </button>
                            )}
                        </div>
                    </div>
                ) : (
                    <div className="space-y-4">
                        <div className="flex items-center gap-3 p-4 bg-amber-50 dark:bg-amber-500/20 border border-amber-200 dark:border-amber-500/30 rounded-lg">
                            <span className="material-symbols-outlined text-amber-600 dark:text-amber-400">warning</span>
                            <div>
                                <p className="text-sm font-medium text-amber-800 dark:text-amber-300">Not connected to GitLab</p>
                                <p className="text-xs text-amber-600 dark:text-amber-400">Link a GitLab repository to enable code sync and MR creation</p>
                            </div>
                        </div>
                        {canLinkGitLab ? (
                            <button
                                onClick={() => setShowGitLabModal(true)}
                                className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-medium rounded-lg transition-colors flex items-center gap-2"
                            >
                                <span className="material-symbols-outlined text-[18px]">link</span>
                                Link GitLab Project
                            </button>
                        ) : (
                            <p className="text-sm text-muted-foreground">
                                Only System Admin can link GitLab. Contact admin to connect repository.
                            </p>
                        )}
                    </div>
                )}
            </div>

            {/* Members (Owner only) */}
            <div className="bg-card border border-border rounded-xl p-6">
                <h3 className="text-lg font-bold text-card-foreground mb-4 flex items-center gap-2">
                    <span className="material-symbols-outlined text-blue-500">group</span>
                    Members
                </h3>
                <ProjectMembersPanel
                    projectId={projectId}
                    canManageMembers={!!canManageMembers}
                    members={members}
                    setMembers={setMembers}
                    loading={membersLoading}
                    onRefresh={onRefresh}
                />
            </div>

            {/* Danger Zone - Owner and Admin only */}
            {canDeleteProject && (
            <div className="bg-card border border-red-200 dark:border-red-500/30 rounded-xl p-6">
                <h3 className="text-lg font-bold text-red-600 dark:text-red-400 mb-4 flex items-center gap-2">
                    <span className="material-symbols-outlined">warning</span>
                    Danger Zone
                </h3>
                <p className="text-sm text-muted-foreground mb-4">
                    These actions are irreversible. Please proceed with caution.
                </p>

                {!showDeleteConfirm ? (
                    <button
                        onClick={() => {
                            setShowDeleteConfirm(true);
                            setDeleteConfirmText('');
                            setDeleteLocalFolder(false);
                            setDeleteGitRepo(false);
                            setDeleteError('');
                        }}
                        className="px-4 py-2 bg-red-100 dark:bg-red-500/20 text-red-600 dark:text-red-400 text-sm font-medium rounded-lg border border-red-200 dark:border-red-500/30 hover:bg-red-200 dark:hover:bg-red-500/30 transition-colors"
                    >
                        Delete Project
                    </button>
                ) : (
                    <div className="space-y-3 p-4 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 rounded-lg">
                        <p className="text-sm text-red-700 dark:text-red-300">
                            This will permanently delete <strong>{projectName}</strong> and all its tasks, attempts, and logs.
                        </p>
                        <div className="space-y-2 rounded-lg border border-red-200/70 dark:border-red-500/30 bg-card/70 p-3">
                            <label className="flex items-start gap-2 text-sm text-card-foreground cursor-pointer">
                                <input
                                    type="checkbox"
                                    checked={deleteLocalFolder}
                                    onChange={(e) => setDeleteLocalFolder(e.target.checked)}
                                    className="mt-0.5 size-4 rounded border-border bg-card text-red-600 focus:ring-red-500"
                                />
                                <span>
                                    Also delete local workspace folder
                                    <span className="block text-xs text-muted-foreground">
                                        Remove cloned code/worktree on this machine.
                                    </span>
                                </span>
                            </label>
                            <label className="flex items-start gap-2 text-sm text-card-foreground cursor-pointer">
                                <input
                                    type="checkbox"
                                    checked={deleteGitRepo}
                                    onChange={(e) => setDeleteGitRepo(e.target.checked)}
                                    disabled={!hasRepositoryLink}
                                    className="mt-0.5 size-4 rounded border-border bg-card text-red-600 focus:ring-red-500"
                                />
                                <span>
                                    Also delete remote Git repository
                                    <span className="block text-xs text-muted-foreground">
                                        {hasRepositoryLink
                                            ? 'Permanently removes the repository from Git provider.'
                                            : 'No linked repository found for this project.'}
                                    </span>
                                </span>
                            </label>
                        </div>
                        <div>
                            <label className="block text-xs text-red-600 dark:text-red-400 mb-1">
                                Type <strong>{projectName}</strong> to confirm:
                            </label>
                            <input
                                type="text"
                                value={deleteConfirmText}
                                onChange={(e) => setDeleteConfirmText(e.target.value)}
                                placeholder={projectName}
                                className="w-full px-3 py-2 text-sm border border-red-300 dark:border-red-500/50 rounded-lg bg-card text-card-foreground focus:outline-none focus:ring-2 focus:ring-red-500"
                            />
                        </div>
                        {deleteError && (
                            <p className="text-xs text-red-600 dark:text-red-400">{deleteError}</p>
                        )}
                        <div className="flex gap-2">
                            <button
                                onClick={handleDelete}
                                disabled={deleteConfirmText !== projectName || deleting}
                                className="px-4 py-2 bg-red-600 text-white text-sm font-medium rounded-lg hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                            >
                                {deleting ? 'Deleting...' : 'Delete Forever'}
                            </button>
                            <button
                                onClick={() => {
                                    setShowDeleteConfirm(false);
                                    setDeleteConfirmText('');
                                    setDeleteLocalFolder(false);
                                    setDeleteGitRepo(false);
                                    setDeleteError('');
                                }}
                                className="px-4 py-2 bg-muted hover:bg-muted/80 text-card-foreground text-sm font-medium rounded-lg transition-colors"
                            >
                                Cancel
                            </button>
                        </div>
                    </div>
                )}
            </div>
            )}

            {/* GitLab Modal */}
            {showGitLabModal && (
                <GitLabIntegrationSettings
                    projectId={projectId}
                    onClose={() => setShowGitLabModal(false)}
                    onSuccess={() => {
                        setShowGitLabModal(false);
                        onRefresh();
                    }}
                />
            )}
        </div>
    );
}
