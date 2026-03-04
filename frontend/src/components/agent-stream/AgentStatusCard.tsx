import type { AgentStatus } from '../../api/agentLogs';

interface AgentStatusCardProps {
    agent: AgentStatus;
    onClick?: () => void;
    onReviewClick?: () => void;
    compact?: boolean;
}

// Format time as relative (e.g., "2m ago", "1h ago")
function formatTimeAgo(dateString: string | null): string {
    if (!dateString) return '';
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    return `${diffDays}d ago`;
}

export function AgentStatusCard({ agent, onClick, onReviewClick, compact = false }: AgentStatusCardProps) {
    const getStatusStyles = () => {
        switch (agent.status) {
            case 'running':
                return {
                    border: 'border-green-500/50',
                    bg: 'bg-green-50 dark:bg-green-500/20',
                    dot: 'bg-green-500',
                    text: 'text-green-600 dark:text-green-400',
                    label: 'Running',
                    icon: 'sync',
                    iconClass: 'animate-spin'
                };
            case 'queued':
                return {
                    border: 'border-blue-500/50',
                    bg: 'bg-blue-50 dark:bg-blue-500/20',
                    dot: 'bg-blue-500',
                    text: 'text-blue-600 dark:text-blue-400',
                    label: 'Queued',
                    icon: 'schedule',
                    iconClass: ''
                };
            case 'success':
                return {
                    border: 'border-emerald-500/30',
                    bg: 'bg-emerald-50/50 dark:bg-emerald-500/20',
                    dot: 'bg-emerald-500',
                    text: 'text-emerald-600 dark:text-emerald-400',
                    label: 'Completed',
                    icon: 'check_circle',
                    iconClass: ''
                };
            case 'failed':
                return {
                    border: 'border-red-500/50',
                    bg: 'bg-red-50 dark:bg-red-500/20',
                    dot: 'bg-red-500',
                    text: 'text-red-600 dark:text-red-400',
                    label: 'Failed',
                    icon: 'error',
                    iconClass: ''
                };
            case 'cancelled':
                return {
                    border: 'border-border',
                    bg: 'bg-muted/50',
                    dot: 'bg-muted-foreground/50',
                    text: 'text-muted-foreground',
                    label: 'Cancelled',
                    icon: 'cancel',
                    iconClass: ''
                };
            default:
                return {
                    border: 'border-border',
                    bg: 'bg-card',
                    dot: 'bg-muted-foreground/50',
                    text: 'text-muted-foreground',
                    label: 'Unknown',
                    icon: 'help',
                    iconClass: ''
                };
        }
    };

    const styles = getStatusStyles();
    const timeAgo = formatTimeAgo(agent.started_at || agent.created_at);

    // Use project name as the main identifier, task title as description
    const displayName = agent.project_name || 'Unknown Project';
    const taskDescription = agent.task_title || 'No task';

    const handleReviewClick = (e: React.MouseEvent) => {
        e.stopPropagation();
        onReviewClick?.();
    };

    if (compact) {
        return (
            <div
                className={`flex items-center gap-3 rounded-lg px-3 py-2 border ${styles.border} ${styles.bg} ${onClick ? 'cursor-pointer hover:shadow-sm transition-shadow' : ''}`}
                onClick={onClick}
            >
                <div className={`w-2 h-2 rounded-full ${styles.dot} ${agent.status === 'running' ? 'animate-pulse' : ''}`} />
                <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-card-foreground truncate">
                        {displayName}
                    </p>
                    <p className="text-xs text-muted-foreground truncate">
                        {taskDescription}
                    </p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                    {agent.status === 'success' && onReviewClick && (
                        <button
                            onClick={handleReviewClick}
                            className="px-2 py-1 text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 rounded transition-colors flex items-center gap-1"
                        >
                            <span className="material-symbols-outlined text-[14px]">difference</span>
                            Review
                        </button>
                    )}
                    <span className={`text-xs font-medium ${styles.text}`}>{styles.label}</span>
                    <span className="text-[10px] text-muted-foreground">{timeAgo}</span>
                </div>
            </div>
        );
    }

    return (
        <div
            className={`flex flex-col rounded-xl p-3 border ${styles.border} ${styles.bg} ${onClick ? 'cursor-pointer hover:shadow-md transition-shadow' : ''}`}
            onClick={onClick}
        >
            {/* Header row with status indicator */}
            <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                    <div className={`w-2 h-2 rounded-full ${styles.dot} ${agent.status === 'running' ? 'animate-pulse' : ''}`} />
                    <span className={`text-xs font-semibold uppercase tracking-wide ${styles.text}`}>
                        {styles.label}
                    </span>
                </div>
                <span className={`material-symbols-outlined ${styles.text} ${styles.iconClass} text-[18px]`}>
                    {styles.icon}
                </span>
            </div>

            {/* Project/Agent name */}
            <p className="text-sm font-semibold text-card-foreground truncate" title={displayName}>
                {displayName}
            </p>

            {/* Task title */}
            <p className="text-xs text-muted-foreground truncate mt-0.5" title={taskDescription}>
                {taskDescription}
            </p>

            {/* Footer with time and review button */}
            <div className="flex items-center justify-between mt-2 pt-2 border-t border-border/50">
                <span className="text-[10px] text-muted-foreground font-mono">
                    {agent.started_at ? new Date(agent.started_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : '--:--'}
                </span>
                <div className="flex items-center gap-2">
                    {agent.status === 'success' && onReviewClick && (
                        <button
                            onClick={handleReviewClick}
                            className="px-2 py-0.5 text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 rounded transition-colors flex items-center gap-1"
                        >
                            <span className="material-symbols-outlined text-[12px]">difference</span>
                            Review
                        </button>
                    )}
                    <span className="text-[10px] text-muted-foreground">
                        {timeAgo}
                    </span>
                </div>
            </div>
        </div>
    );
}
