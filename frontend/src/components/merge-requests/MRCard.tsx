import type { MergeRequest } from '../../api/mergeRequests';

interface MRCardProps {
    mr: MergeRequest;
    onReview?: (mr: MergeRequest) => void;
}

export function MRCard({ mr, onReview }: MRCardProps) {
    const getStatusColor = () => {
        switch (mr.status) {
            case 'merged':
                return 'border-green-500/30 bg-green-50 dark:bg-green-900/10';
            case 'pending_review':
                return 'border-amber-500/30 bg-amber-50 dark:bg-amber-900/10';
            case 'open':
                return 'border-blue-500/30 bg-blue-50 dark:bg-blue-900/10';
            case 'closed':
                return 'border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/10';
            default:
                return 'border-slate-200 dark:border-border-dark';
        }
    };

    const getStatusBadge = () => {
        switch (mr.status) {
            case 'merged':
                return { label: 'Merged', color: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400' };
            case 'pending_review':
                return { label: 'Pending Review', color: 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400' };
            case 'open':
                return { label: 'Open', color: 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400' };
            case 'closed':
                return { label: 'Closed', color: 'bg-slate-100 dark:bg-slate-900/30 text-slate-700 dark:text-slate-400' };
            default:
                return { label: 'Unknown', color: 'bg-slate-100 dark:bg-slate-900/30 text-slate-700 dark:text-slate-400' };
        }
    };

    const statusBadge = getStatusBadge();

    return (
        <div className={`p-6 border-l-4 ${getStatusColor()} hover:shadow-md transition-shadow`}>
            <div className="flex items-start justify-between gap-4 mb-3">
                <div className="flex-1">
                    <div className="flex items-center gap-2 mb-2">
                        <span className="text-xs font-mono text-slate-500 dark:text-slate-400">!{mr.mrNumber}</span>
                        <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${statusBadge.color}`}>
                            {statusBadge.label}
                        </span>
                        {mr.author.isAgent && (
                            <span className="text-xs px-2 py-0.5 rounded-full font-medium bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-400">
                                AI Generated
                            </span>
                        )}
                    </div>
                    <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1">{mr.title}</h3>
                    <p className="text-sm text-slate-600 dark:text-slate-400 mb-3">{mr.description}</p>
                    <div className="flex flex-wrap items-center gap-4 text-xs text-slate-500 dark:text-slate-400">
                        <div className="flex items-center gap-1">
                            <span className="material-symbols-outlined text-[16px]">person</span>
                            <span>{mr.author.name}</span>
                        </div>
                        <div className="flex items-center gap-1">
                            <span className="material-symbols-outlined text-[16px]">folder</span>
                            <span>{mr.projectName}</span>
                        </div>
                        <div className="flex items-center gap-1">
                            <span className="material-symbols-outlined text-[16px]">description</span>
                            <span>{mr.changes.files} files</span>
                        </div>
                        <div className="flex items-center gap-1">
                            <span className="text-green-600 dark:text-green-400">+{mr.changes.additions}</span>
                            <span className="text-red-600 dark:text-red-400">-{mr.changes.deletions}</span>
                        </div>
                        <div className="flex items-center gap-1">
                            <span className="material-symbols-outlined text-[16px]">schedule</span>
                            <span>Updated {mr.updatedAt}</span>
                        </div>
                    </div>
                </div>
                {onReview && mr.status !== 'merged' && mr.status !== 'closed' && (
                    <button
                        onClick={() => onReview(mr)}
                        className="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg transition-colors"
                    >
                        <span className="material-symbols-outlined text-[18px]">rate_review</span>
                        Review
                    </button>
                )}
            </div>
        </div>
    );
}
