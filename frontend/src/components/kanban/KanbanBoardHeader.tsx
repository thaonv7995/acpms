/**
 * KanbanBoardHeader - Stats row and toolbar for Kanban board
 *
 * Displays:
 * - Sprint progress stats
 * - In review count
 * - Active agents count
 * - Project selector
 * - Filter chips (All/Agents only)
 * - Search input
 * - Create task button
 */

import type { ProjectDto } from '../../api/generated/models';

interface KanbanStats {
    sprintProgress: number;
    inReviewTasks: number;
    activeAgents: number;
}

interface KanbanBoardHeaderProps {
    /** Calculated stats from columns */
    stats: KanbanStats;
    /** List of available projects */
    projects: ProjectDto[];
    /** Currently selected project ID */
    selectedProjectId?: string;
    /** Whether projects are loading */
    projectsLoading: boolean;
    /** Called when project selection changes */
    onProjectChange: (projectId: string | undefined) => void;
    /** Whether agent-only filter is active */
    agentOnlyFilter: boolean;
    /** Called when agent filter changes */
    onAgentFilterChange: (agentOnly: boolean) => void;
    /** Current search query */
    searchQuery: string;
    /** Called when search query changes */
    onSearchChange: (query: string) => void;
    /** Called when create task button is clicked */
    onCreateTask: () => void;
}

export function KanbanBoardHeader({
    stats,
    projects,
    selectedProjectId,
    projectsLoading,
    onProjectChange,
    agentOnlyFilter,
    onAgentFilterChange,
    searchQuery,
    onSearchChange,
    onCreateTask,
}: KanbanBoardHeaderProps) {
    return (
        <div className="flex flex-col border-b border-slate-200 dark:border-border-dark bg-white dark:bg-background-dark z-10 shrink-0 shadow-sm">
            {/* Quick Stats Row */}
            <div className="flex items-center gap-6 px-6 py-4 border-b border-slate-200 dark:border-border-dark/50 overflow-x-auto no-scrollbar">
                <div className="flex items-center gap-3 min-w-fit">
                    <span className="material-symbols-outlined text-primary text-3xl">timelapse</span>
                    <div>
                        <p className="text-xs text-slate-500 dark:text-[#9cabba]">Sprint Progress</p>
                        <p className="text-lg font-bold leading-none text-slate-900 dark:text-white">
                            {stats.sprintProgress}%
                        </p>
                    </div>
                </div>
                <div className="w-px h-8 bg-slate-200 dark:bg-border-dark/50"></div>
                <div className="flex items-center gap-3 min-w-fit">
                    <span className="material-symbols-outlined text-primary text-3xl">pending_actions</span>
                    <div>
                        <p className="text-xs text-slate-500 dark:text-[#9cabba]">In Review</p>
                        <p className="text-lg font-bold leading-none text-slate-900 dark:text-white">
                            {stats.inReviewTasks} <span className="text-xs font-normal text-slate-500">tasks</span>
                        </p>
                    </div>
                </div>
                <div className="w-px h-8 bg-slate-200 dark:bg-border-dark/50"></div>
                <div className="flex items-center gap-3 min-w-fit">
                    <span className="material-symbols-outlined text-primary text-3xl">smart_toy</span>
                    <div>
                        <p className="text-xs text-slate-500 dark:text-[#9cabba]">Active Agents</p>
                        <p className="text-lg font-bold leading-none text-slate-900 dark:text-white">
                            {stats.activeAgents} <span className="text-xs font-normal text-slate-500">working</span>
                        </p>
                    </div>
                </div>
            </div>

            {/* Toolbar & Filters */}
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 px-6 py-3">
                <div className="flex flex-col gap-3 w-full md:w-auto">
                    <div className="flex items-center gap-4 flex-wrap">
                        {/* Project Selector */}
                        <div className="relative">
                            <select
                                value={selectedProjectId || ''}
                                onChange={(e) => onProjectChange(e.target.value || undefined)}
                                disabled={projectsLoading}
                                className="appearance-none bg-transparent text-sm font-bold text-slate-900 dark:text-white pr-8 py-1 cursor-pointer focus:outline-none"
                            >
                                <option value="" disabled>Select Project</option>
                                {projects.map((project) => (
                                    <option key={project.id} value={project.id}>
                                        {project.name}
                                    </option>
                                ))}
                            </select>
                            <span className="material-symbols-outlined absolute right-0 top-1/2 -translate-y-1/2 text-lg pointer-events-none">
                                expand_more
                            </span>
                        </div>
                        <div className="h-6 w-px bg-slate-200 dark:bg-border-dark hidden md:block"></div>

                        {/* Filter Chips */}
                        <div className="flex gap-2 overflow-x-auto no-scrollbar pb-1 md:pb-0">
                            <button
                                onClick={() => onAgentFilterChange(false)}
                                className={`flex items-center gap-2 px-3 py-1.5 rounded-full transition-colors text-xs font-medium whitespace-nowrap ${
                                    !agentOnlyFilter
                                        ? 'bg-primary/10 text-primary border border-primary/20'
                                        : 'bg-slate-100 dark:bg-surface-border border border-transparent hover:border-slate-300 dark:hover:border-slate-500 text-slate-700 dark:text-white'
                                }`}
                            >
                                <span className="material-symbols-outlined text-[16px]">list</span> All Tasks
                            </button>
                            <button
                                onClick={() => onAgentFilterChange(true)}
                                className={`flex items-center gap-2 px-3 py-1.5 rounded-full transition-colors text-xs font-medium whitespace-nowrap ${
                                    agentOnlyFilter
                                        ? 'bg-primary/10 text-primary border border-primary/20'
                                        : 'bg-slate-100 dark:bg-surface-border border border-transparent hover:border-slate-300 dark:hover:border-slate-500 text-slate-700 dark:text-white'
                                }`}
                            >
                                <span className="material-symbols-outlined text-[16px]">smart_toy</span> Agents Only
                            </button>
                        </div>
                    </div>

                    {/* Search Input */}
                    <div className="relative w-full md:w-80">
                        <span className="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 text-[20px]">
                            search
                        </span>
                        <input
                            type="text"
                            placeholder="Search tasks..."
                            value={searchQuery}
                            onChange={(e) => onSearchChange(e.target.value)}
                            className="w-full pl-10 pr-4 py-2 bg-slate-100 dark:bg-surface-border border border-slate-200 dark:border-border-dark rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary text-slate-900 dark:text-white placeholder-slate-400"
                        />
                        {searchQuery && (
                            <button
                                onClick={() => onSearchChange('')}
                                className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
                            >
                                <span className="material-symbols-outlined text-[18px]">close</span>
                            </button>
                        )}
                    </div>
                </div>

                <button
                    onClick={onCreateTask}
                    className="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg text-sm font-bold transition-colors shadow-lg shadow-blue-500/20"
                >
                    <span className="material-symbols-outlined text-[20px] material-symbols-filled">add</span>
                    <span className="hidden sm:inline">Create Task</span>
                </button>
            </div>
        </div>
    );
}
