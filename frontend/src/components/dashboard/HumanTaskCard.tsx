// HumanTaskCard Component
import { useNavigate } from 'react-router-dom';
import type { DashboardHumanTaskDoc } from '../../api/generated/models/dashboardHumanTaskDoc';

interface HumanTaskCardProps {
    task: DashboardHumanTaskDoc;
    onReview?: (task: DashboardHumanTaskDoc) => void;
    onApprove?: (task: DashboardHumanTaskDoc) => void;
    onAssign?: (task: DashboardHumanTaskDoc) => void;
}

const typeStyles: Record<string, { bg: string; text: string; label: string }> = {
    blocker: {
        bg: 'bg-orange-100 dark:bg-orange-500/20',
        text: 'text-orange-600 dark:text-orange-400',
        label: 'Blocker',
    },
    approval: {
        bg: 'bg-blue-100 dark:bg-blue-500/20',
        text: 'text-blue-600 dark:text-blue-400',
        label: 'Approval',
    },
    qa: {
        bg: 'bg-purple-100 dark:bg-purple-500/20',
        text: 'text-purple-600 dark:text-purple-400',
        label: 'QA',
    },
    review: {
        bg: 'bg-teal-100 dark:bg-teal-500/20',
        text: 'text-teal-600 dark:text-teal-400',
        label: 'Review',
    },
    feature: {
        bg: 'bg-green-100 dark:bg-green-500/20',
        text: 'text-green-600 dark:text-green-400',
        label: 'Feature',
    },
    refactor: {
        bg: 'bg-amber-100 dark:bg-amber-500/20',
        text: 'text-amber-600 dark:text-amber-400',
        label: 'Refactor',
    },
    docs: {
        bg: 'bg-slate-100 dark:bg-slate-500/20',
        text: 'text-slate-600 dark:text-slate-400',
        label: 'Docs',
    },
    test: {
        bg: 'bg-emerald-100 dark:bg-emerald-500/20',
        text: 'text-emerald-600 dark:text-emerald-400',
        label: 'Test',
    },
    init: {
        bg: 'bg-indigo-100 dark:bg-indigo-500/20',
        text: 'text-indigo-600 dark:text-indigo-400',
        label: 'Init',
    },
    chore: {
        bg: 'bg-stone-100 dark:bg-stone-500/20',
        text: 'text-stone-600 dark:text-stone-400',
        label: 'Chore',
    },
    spike: {
        bg: 'bg-rose-100 dark:bg-rose-500/20',
        text: 'text-rose-600 dark:text-rose-400',
        label: 'Spike',
    },
    small_task: {
        bg: 'bg-cyan-100 dark:bg-cyan-500/20',
        text: 'text-cyan-600 dark:text-cyan-400',
        label: 'Small Task',
    },
    deploy: {
        bg: 'bg-pink-100 dark:bg-pink-500/20',
        text: 'text-pink-600 dark:text-pink-400',
        label: 'Deploy',
    },
    default: {
        bg: 'bg-zinc-100 dark:bg-zinc-500/20',
        text: 'text-zinc-600 dark:text-zinc-400',
        label: 'Task',
    }
};

export function HumanTaskCard({ task, onReview, onApprove, onAssign }: HumanTaskCardProps) {
    const navigate = useNavigate();
    const normalizedType = typeStyles[task.type] ? task.type : 'default';
    const style = typeStyles[normalizedType];
    const isUrgent = normalizedType === 'blocker';

    const buildTaskRoute = (preferLatestAttempt: boolean): string => {
        if (task.projectId) {
            if (preferLatestAttempt) {
                return `/tasks/projects/${task.projectId}/${task.id}/attempts/latest`;
            }
            return `/tasks/projects/${task.projectId}/${task.id}`;
        }
        return `/tasks?taskId=${task.id}`;
    };

    const handleCardClick = () => {
        navigate(buildTaskRoute(false));
    };

    const handleActionClick = (e: React.MouseEvent) => {
        e.stopPropagation(); // Prevent card click

        switch (normalizedType) {
            case 'blocker':
            case 'review':
                if (onReview) {
                    onReview(task);
                } else {
                    navigate(buildTaskRoute(true));
                }
                break;
            case 'approval':
                if (onApprove) {
                    onApprove(task);
                } else {
                    navigate(buildTaskRoute(false));
                }
                break;
            case 'qa':
                if (onAssign) {
                    onAssign(task);
                } else {
                    navigate(buildTaskRoute(false));
                }
                break;
        }
    };

    const getActionLabel = () => {
        switch (normalizedType) {
            case 'blocker':
            case 'review':
                return 'Review Now →';
            case 'approval':
                return 'Approve →';
            case 'qa':
                return 'Assign →';
            default:
                return 'View →';
        }
    };

    return (
        <div
            className="p-3 rounded-lg border border-border hover:border-primary/50 bg-card transition-all cursor-pointer group flex flex-col gap-2"
            onClick={handleCardClick}
        >
            <div className="flex justify-between items-center">
                <div className="flex items-center gap-2">
                    <span className={`text-[10px] uppercase font-bold tracking-wider ${style.text} ${style.bg} px-1.5 py-0.5 rounded-sm`}>
                        {style.label}
                    </span>
                    <span className="text-[10px] text-muted-foreground whitespace-nowrap">
                        {new Date(task.createdAt).toLocaleDateString()}
                    </span>
                </div>
                <button
                    onClick={handleActionClick}
                    className={`text-[11px] font-bold tracking-tight transition-colors flex items-center gap-1 ${isUrgent
                        ? 'text-primary hover:text-primary/80'
                        : 'text-muted-foreground hover:text-primary'
                        }`}
                >
                    {getActionLabel()}
                </button>
            </div>

            <div className="flex-1">
                <h4 className="font-semibold text-sm text-card-foreground leading-tight group-hover:text-primary transition-colors line-clamp-2">
                    {task.title}
                </h4>
            </div>
        </div>
    );
}
