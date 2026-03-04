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
};

export function HumanTaskCard({ task, onReview, onApprove, onAssign }: HumanTaskCardProps) {
    const navigate = useNavigate();
    const normalizedType = ['blocker', 'approval', 'qa', 'review'].includes(task.type)
        ? task.type
        : 'qa';
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

    const truncateText = (text: string, maxWords: number = 10): string => {
        const words = text.split(' ');
        if (words.length <= maxWords) {
            return text;
        }
        return words.slice(0, maxWords).join(' ') + '...';
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
            className="p-4 rounded-lg border border-border hover:border-primary/50 bg-card transition-all cursor-pointer group"
            onClick={handleCardClick}
        >
            <div className="flex justify-between items-start mb-2">
                <span className={`text-xs font-bold ${style.text} ${style.bg} px-2 py-0.5 rounded`}>
                    {style.label}
                </span>
                <span className="text-xs text-muted-foreground">
                    {new Date(task.createdAt).toLocaleString()}
                </span>
            </div>
            <h4 className="font-bold text-sm text-card-foreground mb-1 group-hover:text-primary transition-colors">
                {task.title}
            </h4>
            <p className="text-xs text-muted-foreground mb-1 truncate">{task.projectName}</p>
            <p className="text-xs text-muted-foreground mb-3">{truncateText(task.description)}</p>
            <div className="flex items-center justify-between">
                <div className="flex -space-x-1">
                    <div className="size-6 rounded-full bg-muted border border-card flex items-center justify-center">
                        <span className="material-symbols-outlined text-[12px] text-muted-foreground">person</span>
                    </div>
                </div>
                <button
                    onClick={handleActionClick}
                    className={`text-xs font-medium transition-colors ${isUrgent
                            ? 'text-primary hover:text-primary/80'
                            : 'text-muted-foreground hover:text-primary'
                        }`}
                >
                    {getActionLabel()}
                </button>
            </div>
        </div>
    );
}
