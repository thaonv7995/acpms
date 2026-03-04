import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell } from '../components/layout/AppShell';
import { MRCard } from '../components/merge-requests';
import { useMergeRequests } from '../hooks/useMergeRequests';
import { useDebouncedValue } from '../hooks/useDebouncedValue';
import { useToast } from '../hooks/useToast';
import { Toast } from '../components/shared/Toast';
import type { MergeRequest, MRStatus } from '../api/mergeRequests';
import { logger } from '@/lib/logger';

export function MergeRequestPage() {
    const [searchQuery, setSearchQuery] = useState('');
    const [activeTab, setActiveTab] = useState<MRStatus | 'all'>('all');
    const navigate = useNavigate();

    // Debounce search query
    const debouncedSearch = useDebouncedValue(searchQuery, 300);

    const { stats, mergeRequests, loading, syncing, error, search, filterByStatus, syncWithGitLab } = useMergeRequests();
    const { toasts, showToast, hideToast } = useToast();

    // Update search when debounced value changes
    useEffect(() => {
        search(debouncedSearch);
    }, [debouncedSearch, search]);

    const handleTabChange = (tab: MRStatus | 'all') => {
        setActiveTab(tab);
        filterByStatus(tab === 'all' ? null : tab);
    };

    const handleSync = async () => {
        try {
            await syncWithGitLab();
            showToast('Successfully synced with GitLab', 'success');
        } catch (err) {
            showToast('Failed to sync with GitLab', 'error');
            logger.error('Failed to sync with GitLab:', err);
        }
    };

    const handleReview = (mr: MergeRequest) => {
        if (mr.latestAttemptId) {
            navigate(`/tasks/projects/${mr.projectId}/${mr.taskId}/attempts/${mr.latestAttemptId}?view=diffs`);
            return;
        }

        navigate(`/tasks/${mr.taskId}`);
    };

    if (loading) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center">
                    <div className="text-center">
                        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary mx-auto mb-4"></div>
                        <p className="text-slate-500 dark:text-slate-400">Loading merge requests...</p>
                    </div>
                </div>
            </AppShell>
        );
    }

    if (error) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center">
                    <div className="text-center">
                        <span className="material-symbols-outlined text-red-500 text-5xl mb-4">error</span>
                        <p className="text-red-500 mb-2">Failed to load merge requests</p>
                        <p className="text-slate-500 dark:text-slate-400 text-sm">{error}</p>
                    </div>
                </div>
            </AppShell>
        );
    }

    return (
        <AppShell>
            <div className="flex-1 overflow-y-auto p-6 md:p-8 scrollbar-hide">
                <div className="max-w-[1600px] mx-auto flex flex-col gap-6">
                    {/* Header */}
                    <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
                        <div>
                            <h1 className="text-3xl font-bold text-slate-900 dark:text-white mb-2">Merge Requests</h1>
                            <p className="text-slate-500 dark:text-slate-400 text-sm">
                                Review and manage code changes from AI agents and team members.
                            </p>
                        </div>
                        <div className="flex gap-3">
                            <div className="relative">
                                <span className="material-symbols-outlined absolute left-2.5 top-2.5 text-slate-400 text-[18px]">search</span>
                                <input
                                    className="w-64 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark text-sm text-slate-900 dark:text-white rounded-lg pl-9 pr-9 py-2.5 focus:outline-none focus:ring-1 focus:ring-primary placeholder-slate-400"
                                    placeholder="Search merge requests..."
                                    type="text"
                                    value={searchQuery}
                                    onChange={(e) => setSearchQuery(e.target.value)}
                                />
                                {searchQuery && (
                                    <button
                                        onClick={() => setSearchQuery('')}
                                        className="absolute right-2.5 top-2.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
                                    >
                                        <span className="material-symbols-outlined text-[18px]">close</span>
                                    </button>
                                )}
                            </div>
                            <button
                                onClick={handleSync}
                                disabled={syncing}
                                className="flex items-center gap-2 px-4 py-2.5 bg-primary hover:bg-primary/90 disabled:bg-blue-400 text-white text-sm font-bold rounded-lg shadow-lg shadow-primary/20 transition-all disabled:cursor-not-allowed"
                            >
                                <span className={`material-symbols-outlined text-[18px] ${syncing ? 'animate-spin' : ''}`}>
                                    refresh
                                </span>
                                {syncing ? 'Syncing...' : 'Sync with GitLab'}
                            </button>
                        </div>
                    </div>

                    {/* Stats */}
                    {stats && (
                        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
                            <div className="p-5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm">
                                <div className="flex items-center gap-3">
                                    <div className="p-2 rounded-lg bg-blue-100 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400">
                                        <span className="material-symbols-outlined">call_split</span>
                                    </div>
                                    <div>
                                        <p className="text-sm text-slate-500 dark:text-slate-400">Open</p>
                                        <p className="text-2xl font-bold text-slate-900 dark:text-white">{stats.open}</p>
                                    </div>
                                </div>
                            </div>
                            <div className="p-5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm">
                                <div className="flex items-center gap-3">
                                    <div className="p-2 rounded-lg bg-amber-100 dark:bg-amber-500/20 text-amber-600 dark:text-amber-400">
                                        <span className="material-symbols-outlined">pending</span>
                                    </div>
                                    <div>
                                        <p className="text-sm text-slate-500 dark:text-slate-400">Pending Review</p>
                                        <p className="text-2xl font-bold text-slate-900 dark:text-white">{stats.pendingReview}</p>
                                    </div>
                                </div>
                            </div>
                            <div className="p-5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm">
                                <div className="flex items-center gap-3">
                                    <div className="p-2 rounded-lg bg-green-100 dark:bg-green-500/20 text-green-600 dark:text-green-400">
                                        <span className="material-symbols-outlined">check_circle</span>
                                    </div>
                                    <div>
                                        <p className="text-sm text-slate-500 dark:text-slate-400">Merged</p>
                                        <p className="text-2xl font-bold text-slate-900 dark:text-white">{stats.merged}</p>
                                    </div>
                                </div>
                            </div>
                            <div className="p-5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm">
                                <div className="flex items-center gap-3">
                                    <div className="p-2 rounded-lg bg-purple-100 dark:bg-purple-500/20 text-purple-600 dark:text-purple-400">
                                        <span className="material-symbols-outlined">smart_toy</span>
                                    </div>
                                    <div>
                                        <p className="text-sm text-slate-500 dark:text-slate-400">AI Generated</p>
                                        <p className="text-2xl font-bold text-slate-900 dark:text-white">{stats.aiGenerated}</p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    )}

                    {/* MR List */}
                    <div className="bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl overflow-hidden shadow-sm">
                        <div className="px-6 py-4 border-b border-slate-200 dark:border-border-dark flex items-center gap-4 overflow-x-auto">
                            <button
                                onClick={() => handleTabChange('all')}
                                className={`text-sm font-bold whitespace-nowrap pb-1 ${
                                    activeTab === 'all'
                                        ? 'text-primary border-b-2 border-primary'
                                        : 'text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-300'
                                }`}
                            >
                                All ({(stats?.open || 0) + (stats?.pendingReview || 0) + (stats?.merged || 0)})
                            </button>
                            <button
                                onClick={() => handleTabChange('open')}
                                className={`text-sm font-bold whitespace-nowrap pb-1 ${
                                    activeTab === 'open'
                                        ? 'text-primary border-b-2 border-primary'
                                        : 'text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-300'
                                }`}
                            >
                                Open ({stats?.open || 0})
                            </button>
                            <button
                                onClick={() => handleTabChange('pending_review')}
                                className={`text-sm font-bold whitespace-nowrap pb-1 ${
                                    activeTab === 'pending_review'
                                        ? 'text-primary border-b-2 border-primary'
                                        : 'text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-300'
                                }`}
                            >
                                Pending ({stats?.pendingReview || 0})
                            </button>
                            <button
                                onClick={() => handleTabChange('merged')}
                                className={`text-sm font-bold whitespace-nowrap pb-1 ${
                                    activeTab === 'merged'
                                        ? 'text-primary border-b-2 border-primary'
                                        : 'text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-300'
                                }`}
                            >
                                Merged ({stats?.merged || 0})
                            </button>
                        </div>

                        <div className="divide-y divide-slate-200 dark:divide-border-dark">
                            {mergeRequests.length === 0 ? (
                                <div className="p-12 text-center text-slate-500 dark:text-slate-400">
                                    No merge requests found
                                </div>
                            ) : (
                                mergeRequests.map(mr => (
                                    <MRCard key={mr.id} mr={mr} onReview={handleReview} />
                                ))
                            )}
                        </div>
                    </div>
                </div>
            </div>

            {/* Toast Notifications */}
            {toasts.map(toast => (
                <Toast
                    key={toast.id}
                    message={toast.message}
                    type={toast.type}
                    onClose={() => hideToast(toast.id)}
                />
            ))}
        </AppShell>
    );
}
