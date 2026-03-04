import { useState, useEffect } from 'react';
import { getAttemptDiff, DiffResponse } from '../../api/taskAttempts';
import { DiffCard } from './DiffCard';
import '@git-diff-view/react/styles/diff-view.css';
import { logger } from '@/lib/logger';

interface CodeDiffSectionProps {
    attemptId: string;
    collapsed?: boolean;
}

export function CodeDiffSection({ attemptId, collapsed = false }: CodeDiffSectionProps) {
    const [diff, setDiff] = useState<DiffResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [isExpanded, setIsExpanded] = useState(!collapsed);
    const [allCollapsed, setAllCollapsed] = useState(false);

    useEffect(() => {
        fetchDiff();
    }, [attemptId]);

    const fetchDiff = async () => {
        try {
            setLoading(true);
            setError(null);
            const data = await getAttemptDiff(attemptId);
            setDiff(data);
        } catch (err) {
            logger.error('Failed to fetch diff:', err);
            setError('Failed to load diff');
        } finally {
            setLoading(false);
        }
    };

    const toggleAllFiles = () => {
        setAllCollapsed(!allCollapsed);
    };

    return (
        <div className="bg-white dark:bg-surface-dark rounded-xl border border-slate-200 dark:border-slate-700 overflow-hidden">
            {/* Header */}
            <button
                onClick={() => setIsExpanded(!isExpanded)}
                className="w-full px-6 py-4 border-b border-slate-200 dark:border-slate-700 flex items-center justify-between hover:bg-slate-50 dark:hover:bg-slate-800/50 transition-colors"
            >
                <div className="flex items-center gap-3">
                    <span className="material-symbols-outlined text-[18px] text-slate-500">difference</span>
                    <h3 className="text-sm font-bold text-slate-900 dark:text-white uppercase">
                        Code Changes
                    </h3>
                    {diff && (
                        <div className="flex items-center gap-3 text-xs">
                            <span className="text-slate-500">{diff.total_files} files</span>
                            <span className="text-green-500">+{diff.total_additions}</span>
                            <span className="text-red-500">-{diff.total_deletions}</span>
                        </div>
                    )}
                </div>
                <span className={`material-symbols-outlined text-slate-400 transition-transform ${isExpanded ? 'rotate-180' : ''}`}>
                    expand_more
                </span>
            </button>

            {/* Diff content */}
            {isExpanded && (
                <div className="p-4">
                    {/* Actions bar */}
                    {diff && diff.files.length > 0 && (
                        <div className="flex justify-end mb-3">
                            <button
                                onClick={toggleAllFiles}
                                className="text-xs text-slate-500 hover:text-slate-700 dark:hover:text-slate-300 flex items-center gap-1"
                            >
                                <span className="material-symbols-outlined text-[14px]">
                                    {allCollapsed ? 'unfold_more' : 'unfold_less'}
                                </span>
                                {allCollapsed ? 'Expand All' : 'Collapse All'}
                            </button>
                        </div>
                    )}

                    {loading ? (
                        <div className="py-12 text-center text-slate-500">
                            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary mx-auto mb-2"></div>
                            Loading diff...
                        </div>
                    ) : error ? (
                        <div className="py-12 text-center text-red-400">
                            <span className="material-symbols-outlined text-3xl mb-2 block">error</span>
                            {error}
                        </div>
                    ) : !diff || diff.files.length === 0 ? (
                        <div className="py-12 text-center text-slate-500">
                            <span className="material-symbols-outlined text-3xl mb-2 block">code_off</span>
                            No changes to display
                        </div>
                    ) : (
                        <div>
                            {diff.files.map((file, index) => (
                                <DiffCard
                                    key={file.new_path || file.old_path || index}
                                    diff={file}
                                    defaultExpanded={!allCollapsed}
                                />
                            ))}
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
