interface TaskDetailHeaderProps {
    taskId: string;
    title: string;
    status: string;
    displayStatus: string;
    statusColor: string;
    isInReview: boolean;
    hasReviewableAttempt: boolean;
    hasAttempts: boolean;
    onBack: () => void;
    onReviewChanges: () => void;
    onStartAgent: () => void;
    onViewAttempts: () => void;
}

export function TaskDetailHeader({
    taskId,
    title,
    displayStatus,
    statusColor,
    isInReview,
    hasReviewableAttempt,
    hasAttempts,
    onBack,
    onReviewChanges,
    onStartAgent,
    onViewAttempts,
}: TaskDetailHeaderProps) {
    return (
        <div className="flex flex-col md:flex-row md:items-start justify-between gap-4">
            <div className="flex items-start gap-3">
                <button
                    onClick={onBack}
                    className="mt-1 text-muted-foreground hover:text-primary transition-colors"
                >
                    <span className="material-symbols-outlined">arrow_back</span>
                </button>
                <div className="flex-1">
                    <div className="flex items-center gap-3 mb-2">
                        <span className="font-mono text-xs text-muted-foreground">
                            {taskId.slice(0, 8)}
                        </span>
                        <span className="h-4 w-px bg-border"></span>
                        <div className="flex items-center gap-2">
                            <span className={`size-2 rounded-full ${statusColor}`}></span>
                            <span className="text-xs font-bold text-card-foreground uppercase tracking-wider">
                                {displayStatus}
                            </span>
                        </div>
                    </div>
                    <h1 className="text-2xl md:text-3xl font-bold text-card-foreground leading-tight">
                        {title}
                    </h1>
                </div>
            </div>
            <div className="flex items-center gap-2">
                {/* View Attempts Button */}
                {hasAttempts && (
                    <button
                        onClick={onViewAttempts}
                        className="px-3 py-2 bg-card border border-border hover:bg-muted text-card-foreground text-xs font-medium rounded-lg flex items-center gap-1.5 transition-all"
                    >
                        <span className="material-symbols-outlined text-[16px]">terminal</span>
                        View Logs
                    </button>
                )}

                {isInReview ? (
                    <button
                        onClick={onReviewChanges}
                        disabled={!hasReviewableAttempt}
                        className="px-4 py-2 bg-yellow-500 hover:bg-yellow-600 disabled:bg-yellow-400 disabled:cursor-not-allowed text-white text-sm font-bold rounded-lg shadow-lg shadow-yellow-500/20 flex items-center gap-2 transition-all"
                    >
                        <span className="material-symbols-outlined text-[18px]">rate_review</span>
                        Review Changes
                    </button>
                ) : (
                    <button
                        onClick={onStartAgent}
                        className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all"
                    >
                        <span className="material-symbols-outlined text-[18px]">smart_toy</span>
                        Start Agent
                    </button>
                )}
                <button className="p-2 text-muted-foreground hover:text-card-foreground transition-colors rounded-lg hover:bg-muted">
                    <span className="material-symbols-outlined text-[18px]">more_horiz</span>
                </button>
            </div>
        </div>
    );
}
