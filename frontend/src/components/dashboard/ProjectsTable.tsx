// ProjectsTable Component
import { useNavigate } from 'react-router-dom';
import { Link } from 'react-router-dom';
import type { DashboardProjectDoc } from '../../api/generated/models/dashboardProjectDoc';

interface ProjectsTableProps {
    projects: DashboardProjectDoc[];
    onViewAll?: () => void;
}

const statusStyles: Record<string, { bg: string; text: string; label: string }> = {
    planning: {
        bg: 'bg-slate-100 dark:bg-slate-500/20',
        text: 'text-slate-800 dark:text-slate-300',
        label: 'Planning'
    },
    active: {
        bg: 'bg-blue-100 dark:bg-blue-500/20',
        text: 'text-blue-800 dark:text-blue-300',
        label: 'Active'
    },
    reviewing: {
        bg: 'bg-amber-100 dark:bg-amber-500/20',
        text: 'text-amber-800 dark:text-amber-300',
        label: 'Reviewing'
    },
    blocked: {
        bg: 'bg-red-100 dark:bg-red-500/20',
        text: 'text-red-800 dark:text-red-300',
        label: 'Blocked'
    },
    completed: {
        bg: 'bg-green-100 dark:bg-green-500/20',
        text: 'text-green-800 dark:text-green-300',
        label: 'Completed'
    },
    paused: {
        bg: 'bg-slate-100 dark:bg-slate-500/20',
        text: 'text-slate-800 dark:text-slate-300',
        label: 'Paused'
    },
    archived: {
        bg: 'bg-slate-100 dark:bg-slate-500/20',
        text: 'text-slate-800 dark:text-slate-300',
        label: 'Archived'
    },
};

// progressColors removed - now using getProgressColor function instead

// Get progress color based on progress value
const getProgressColor = (progress: number): string => {
    if (progress === 0) {
        return 'bg-muted-foreground/40'; // Gray for 0%
    } else if (progress === 100) {
        return 'bg-green-500'; // Green for 100%
    } else if (progress < 30) {
        return 'bg-red-500'; // Red for low progress
    } else if (progress < 70) {
        return 'bg-yellow-500'; // Yellow for medium progress
    } else {
        return 'bg-blue-500'; // Blue for high progress
    }
};

export function ProjectsTable({ projects, onViewAll }: ProjectsTableProps) {
    const navigate = useNavigate();

    const truncateText = (text: string, maxWords: number = 10): string => {
        const words = text.split(' ');
        if (words.length <= maxWords) {
            return text;
        }
        return words.slice(0, maxWords).join(' ') + '...';
    };

    const handleRowClick = (projectId: string) => {
        navigate(`/projects/${projectId}`);
    };

    return (
        <div className="rounded-xl bg-card border border-border overflow-hidden shadow-sm">
            <div className="px-6 py-4 border-b border-border flex justify-between items-center">
                <h3 className="font-bold text-lg text-card-foreground">Active Projects</h3>
                <Link
                    to="/projects"
                    onClick={onViewAll}
                    className="text-primary text-sm font-medium hover:underline"
                >
                    View All
                </Link>
            </div>
            <div className="overflow-x-auto">
                <table className="w-full text-left border-collapse">
                    <thead>
                        <tr className="text-xs text-muted-foreground uppercase tracking-wider border-b border-border bg-muted/50">
                            <th className="px-6 py-3 font-semibold">Project Name</th>
                            <th className="px-6 py-3 font-semibold">Status</th>
                            <th className="px-6 py-3 font-semibold">Progress</th>
                            <th className="px-6 py-3 font-semibold text-right">Agents</th>
                        </tr>
                    </thead>
                    <tbody className="text-sm">
                        {projects.map((project) => {
                            const status = statusStyles[project.status] ?? statusStyles.planning;
                            const progressValue = Math.max(0, Math.min(100, project.progress || 0));
                            const progressBarColor = getProgressColor(progressValue);

                            return (
                                <tr
                                    key={project.id}
                                    onClick={() => handleRowClick(project.id)}
                                    className="group hover:bg-muted/30 transition-colors border-b border-border last:border-0 cursor-pointer"
                                >
                                    <td className="px-6 py-4 font-medium text-card-foreground group-hover:text-primary transition-colors">
                                        {project.name}
                                        <span className="block text-xs text-muted-foreground font-normal mt-0.5">{truncateText(project.subtitle)}</span>
                                    </td>
                                    <td className="px-6 py-4">
                                        <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${status.bg} ${status.text}`}>
                                            {status.label}
                                        </span>
                                    </td>
                                    <td className="px-6 py-4 w-1/3">
                                        <div className="flex items-center gap-3">
                                            <div className="flex-1 h-1.5 bg-muted dark:bg-muted/50 rounded-full overflow-hidden relative">
                                                {progressValue > 0 ? (
                                                    <div 
                                                        className={`h-full ${progressBarColor} rounded-full transition-all duration-300 ease-out`} 
                                                        style={{ 
                                                            width: `${progressValue}%`,
                                                            minWidth: '2px'
                                                        }}
                                                    ></div>
                                                ) : (
                                                    <div className="h-full w-full border border-muted-foreground/30 rounded-full"></div>
                                                )}
                                            </div>
                                            <span className="text-xs font-medium text-muted-foreground whitespace-nowrap min-w-[2.5rem] text-right">
                                                {progressValue}%
                                            </span>
                                        </div>
                                    </td>
                                    <td className="px-6 py-4 text-right">
                                        <div className="flex justify-end -space-x-2">
                                            {(project.agents || []).map((agent) => (
                                                <div
                                                    key={agent.id}
                                                    className={`size-7 rounded-full ${agent.color?.startsWith('bg-') ? agent.color : 'bg-blue-500'} border-2 border-card flex items-center justify-center text-[10px] text-white font-bold`}
                                                >
                                                    {agent.initial}
                                                </div>
                                            ))}
                                        </div>
                                    </td>
                                </tr>
                            );
                        })}
                    </tbody>
                </table>
            </div>
        </div>
    );
}
