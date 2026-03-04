interface TaskActionsProps {
    status?: string;
    onStartAgent?: () => void;
    onReviewChanges?: () => void;
    onStatusChange?: (status: string) => void;
    reviewDisabled?: boolean;
}

export function TaskActions({ status, onStartAgent, onReviewChanges, onStatusChange, reviewDisabled }: TaskActionsProps) {
    const isInReview = status === 'in_review';

    return (
        <div className="flex flex-col gap-3">
            {isInReview ? (
                <button
                    onClick={onReviewChanges}
                    disabled={reviewDisabled}
                    className="w-full py-2.5 px-4 bg-yellow-500 hover:bg-yellow-600 disabled:bg-yellow-400 disabled:cursor-not-allowed text-white text-sm font-bold rounded-lg shadow-lg shadow-yellow-500/20 flex items-center justify-center gap-2 transition-all active:scale-95"
                >
                    {reviewDisabled ? (
                        <>
                            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                            Loading...
                        </>
                    ) : (
                        <>
                            <span className="material-symbols-outlined text-[20px]">rate_review</span>
                            Review Changes
                        </>
                    )}
                </button>
            ) : (
                <button
                    onClick={onStartAgent}
                    className="w-full py-2.5 px-4 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center justify-center gap-2 transition-all active:scale-95"
                >
                    <span className="material-symbols-outlined text-[20px]">smart_toy</span>
                    Start Agent
                </button>
            )}
            <select
                onChange={(e) => onStatusChange?.(e.target.value)}
                className="w-full bg-white dark:bg-[#0d1117] border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 text-sm rounded-lg py-2.5 px-3 focus:ring-primary focus:border-primary"
                defaultValue=""
            >
                <option value="" disabled>Change Status</option>
                <option value="todo">Move to To Do</option>
                <option value="in_progress">Move to In Progress</option>
                {isInReview && <option value="done">Approve & Move to Done</option>}
                <option value="done">Move to Done</option>
            </select>
        </div>
    );
}
