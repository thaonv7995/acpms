import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { KanbanColumn } from './KanbanColumn';
import { KanbanProvider } from './KanbanProvider';
import { useDebouncedValue } from '../../hooks/useDebouncedValue';
import { useKanbanStats } from '../../hooks/useKanbanStats';
import type { KanbanColumn as KanbanColumnType, KanbanTask } from '../../types/project';
import type { CreatedDateFilter } from '../../mappers/taskMapper';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
} from '../ui/dropdown-menu';
import { logger } from '@/lib/logger';

interface Project {
  id: string;
  name: string;
}

interface KanbanBoardProps {
  columns: KanbanColumnType[];
  loading: boolean;
  onTaskClick: (task: KanbanTask) => void;
  onCreateTask?: () => void;
  onFiltersChange?: (filters: {
    agentOnly?: boolean;
    search?: string;
    createdDate?: CreatedDateFilter;
  }) => void;
  onTaskMove?: (taskId: string, newStatus: string) => Promise<void>;
  selectedTaskId?: string | null;
  projects?: Project[];
  selectedProjectId?: string;
  onProjectChange?: (projectId: string) => void;
  onTaskStart?: (taskId: string) => Promise<void> | void;
  onTaskCancelExecution?: (taskId: string) => Promise<void> | void;
  onTaskDelete?: (taskId: string) => Promise<void> | void;
  onTaskViewDetails?: (taskId: string) => void;
  onTaskEdit?: (taskId: string) => void;
  onTaskNewAttempt?: (taskId: string) => void;
  onTaskRetry?: (taskId: string) => Promise<void> | void;
  onTaskClose?: (taskId: string) => Promise<void> | void;
  onCloseAllDone?: () => Promise<void> | void;
  /** Raw TaskDto[] from useKanban — pass to useKanbanStats to avoid duplicate API calls */
  rawTasks?: import('../../api/generated/models').TaskDto[];
}

export function KanbanBoard({
  columns,
  loading: _loading,
  onTaskClick,
  onCreateTask,
  onFiltersChange,
  onTaskMove,
  selectedTaskId,
  projects = [],
  selectedProjectId,
  onProjectChange,
  onTaskStart,
  onTaskCancelExecution,
  onTaskDelete,
  onTaskViewDetails,
  onTaskEdit,
  onTaskNewAttempt,
  onTaskRetry,
  onTaskClose,
  onCloseAllDone,
  rawTasks,
}: KanbanBoardProps) {
  const navigate = useNavigate();
  const [agentOnlyFilter, setAgentOnlyFilter] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [createdDateFilter, setCreatedDateFilter] = useState<CreatedDateFilter>('all');
  const debouncedSearch = useDebouncedValue(searchQuery, 300);

  const isAllProjects = selectedProjectId === 'all' || !selectedProjectId;
  const selectedProject = projects.find(p => p.id === selectedProjectId);
  const projectName = isAllProjects ? 'All Projects' : (selectedProject?.name || 'Select Project');

  // Determine if we should show project name chips
  const showProjectChips = isAllProjects;

  // Fetch real stats — pass rawTasks to avoid duplicate API calls
  const effectiveProjectIdForStats = isAllProjects ? undefined : selectedProjectId;
  const { stats: kanbanStats, loading: statsLoading } = useKanbanStats(effectiveProjectIdForStats, rawTasks);

  const createdDateLabels: Record<CreatedDateFilter, string> = {
    all: 'Any time',
    today: 'Today',
    this_week: 'This week',
    this_month: 'This month',
    last_30_days: 'Last 30 days',
  };

  const handleProjectSelect = useCallback((projectId: string) => {
    if (onProjectChange) {
      onProjectChange(projectId);
    } else {
      // Default: navigate to project tasks page
      if (projectId === 'all') {
        navigate('/tasks');
      } else {
        navigate(`/projects/${projectId}/tasks`);
      }
    }
  }, [onProjectChange, navigate]);

  // Auto-switch to "All Tasks" when the selected task is filtered out by Execution Only
  useEffect(() => {
    if (selectedTaskId && agentOnlyFilter) {
      const found = columns.some(col => col.tasks.some(t => t.id === selectedTaskId));
      if (!found) {
        setAgentOnlyFilter(false);
      }
    }
  }, [selectedTaskId, columns, agentOnlyFilter]);

  useEffect(() => {
    onFiltersChange?.({
      agentOnly: agentOnlyFilter,
      search: debouncedSearch,
      createdDate: createdDateFilter,
    });
  }, [debouncedSearch, agentOnlyFilter, createdDateFilter, onFiltersChange]);

  // Handle task move with error handling
  const handleTaskMove = useCallback(
    async (taskId: string, newStatus: string) => {
      try {
        await onTaskMove?.(taskId, newStatus);
      } catch (err) {
        logger.error('Failed to move task:', err);
        // Parent component handles error display
      }
    },
    [onTaskMove]
  );

  // Don't show loading spinner - render empty columns instead
  // This provides better UX as users see the structure immediately

  return (
    <div className="h-full min-h-0 flex flex-col min-w-0 overflow-hidden">
      {/* Header Section */}
      <div className="flex flex-col border-b border-border bg-card z-10 shrink-0">
        {/* Quick Stats Row - simplified */}
        <div className="flex items-center gap-6 px-6 py-4 border-b border-border/50 overflow-x-auto">
          <div className="flex items-center gap-3 min-w-fit">
            <span className="material-symbols-outlined text-primary text-3xl">pending_actions</span>
            <div>
              <p className="text-xs text-slate-500 dark:text-slate-400">Open Tasks</p>
              <p className="text-lg font-bold leading-none text-slate-900 dark:text-white">
                {statsLoading ? '...' : kanbanStats.openTasks}
              </p>
            </div>
          </div>
          <div className="w-px h-8 bg-slate-200 dark:bg-slate-700/50"></div>
          <div className="flex items-center gap-3 min-w-fit">
            <span className="material-symbols-outlined text-primary text-3xl">trending_up</span>
            <div>
              <p className="text-xs text-slate-500 dark:text-slate-400">{kanbanStats.sprintProgress.label}</p>
              <p className="text-lg font-bold leading-none text-slate-900 dark:text-white">
                {statsLoading ? '...' : (
                  <>
                    {kanbanStats.sprintProgress.percentage}%
                    {kanbanStats.sprintProgress.trend && (
                      <span className="text-xs font-medium text-green-500 ml-1">
                        {kanbanStats.sprintProgress.trend}
                      </span>
                    )}
                  </>
                )}
              </p>
            </div>
          </div>
          <div className="w-px h-8 bg-slate-200 dark:bg-slate-700/50"></div>
          <div className="flex items-center gap-3 min-w-fit">
            <span className="material-symbols-outlined text-primary text-3xl">bolt</span>
            <div>
              <p className="text-xs text-slate-500 dark:text-slate-400">Agent Velocity</p>
              <p className="text-lg font-bold leading-none text-slate-900 dark:text-white">
                {statsLoading ? '...' : (
                  <>
                    {kanbanStats.agentVelocity.tasksPerHour}{' '}
                    <span className="text-xs font-normal text-slate-500">tasks/day</span>
                  </>
                )}
              </p>
            </div>
          </div>
          <div className="w-px h-8 bg-slate-200 dark:bg-slate-700/50"></div>
          <div className="flex items-center gap-3 min-w-fit">
            <span className="material-symbols-outlined text-primary text-3xl">smart_toy</span>
            <div>
              <p className="text-xs text-slate-500 dark:text-slate-400">Active Agents</p>
              <p className="text-lg font-bold leading-none text-slate-900 dark:text-white">
                {statsLoading ? '...' : (
                  <>
                    {kanbanStats.activeAgents.count}{' '}
                    <span className="text-xs font-normal text-slate-500">{kanbanStats.activeAgents.label}</span>
                  </>
                )}
              </p>
            </div>
          </div>
        </div>

        {/* Toolbar & Filters */}
        <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 px-6 py-3">
          <div className="flex items-center gap-4 flex-wrap w-full md:w-auto">
            {projects.length > 0 ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button className="flex items-center gap-2 text-sm font-bold text-slate-900 dark:text-white hover:opacity-80 transition-opacity">
                    {projectName}
                    <span className="material-symbols-outlined text-lg">expand_more</span>
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="w-56">
                  <DropdownMenuItem
                    onClick={(e) => {
                      e.stopPropagation();
                      handleProjectSelect('all');
                    }}
                    className={isAllProjects ? 'bg-primary/10 text-primary' : ''}
                  >
                    All Projects
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  {projects.map((project) => (
                    <DropdownMenuItem
                      key={project.id}
                      onClick={(e) => {
                        e.stopPropagation();
                        handleProjectSelect(project.id);
                      }}
                      className={selectedProjectId === project.id ? 'bg-primary/10 text-primary' : ''}
                    >
                      {project.name}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <button className="flex items-center gap-2 text-sm font-bold text-slate-900 dark:text-white">
                {projectName}
                <span className="material-symbols-outlined text-lg">expand_more</span>
              </button>
            )}
            <div className="h-6 w-px bg-slate-200 dark:bg-slate-700 hidden md:block"></div>
            {/* Filter Chips */}
            <div className="flex gap-2 overflow-x-auto pb-1 md:pb-0">
              <button
                onClick={() => setAgentOnlyFilter(false)}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-full transition-colors text-xs font-bold whitespace-nowrap ${!agentOnlyFilter
                    ? 'bg-primary text-primary-foreground border border-primary shadow-sm'
                    : 'bg-primary/20 dark:bg-primary/30 border border-primary/30 text-card-foreground hover:bg-primary/30 dark:hover:bg-primary/40'
                  }`}
              >
                <span className="material-symbols-outlined text-[16px]">list</span> All Tasks
              </button>
              <button
                onClick={() => setAgentOnlyFilter(true)}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-full transition-colors text-xs font-bold whitespace-nowrap ${agentOnlyFilter
                    ? 'bg-primary text-primary-foreground border border-primary shadow-sm'
                    : 'bg-primary/20 dark:bg-primary/30 border border-primary/30 text-card-foreground hover:bg-primary/30 dark:hover:bg-primary/40'
                  }`}
                title="Show execution tasks only (exclude Docs/Spike/Init tasks)"
              >
                <span className="material-symbols-outlined text-[16px]">code</span> Execution Only
              </button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    className="flex items-center gap-2 px-3 py-1.5 rounded-full transition-colors text-xs font-bold whitespace-nowrap bg-primary/20 dark:bg-primary/30 border border-primary/30 text-card-foreground hover:bg-primary/30 dark:hover:bg-primary/40"
                    title="Filter by created date"
                  >
                    <span className="material-symbols-outlined text-[16px]">calendar_month</span>
                    {createdDateLabels[createdDateFilter]}
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="w-44">
                  <DropdownMenuItem onClick={() => setCreatedDateFilter('all')}>
                    Any time
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => setCreatedDateFilter('today')}>
                    Today
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => setCreatedDateFilter('this_week')}>
                    This week
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => setCreatedDateFilter('this_month')}>
                    This month
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => setCreatedDateFilter('last_30_days')}>
                    Last 30 days
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
            {/* Search Input */}
            <div className="relative flex-1 md:w-80 min-w-[200px]">
              <span className="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-card-foreground text-[18px]">
                search
              </span>
              <input
                type="text"
                placeholder="Search tasks..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-primary/10 dark:bg-primary/20 border border-primary/30 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary text-card-foreground placeholder:text-card-foreground/60"
              />
              {searchQuery && (
                <button
                  onClick={() => setSearchQuery('')}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-card-foreground/70 hover:text-card-foreground transition-colors"
                >
                  <span className="material-symbols-outlined text-[16px]">close</span>
                </button>
              )}
            </div>
          </div>
          {onCreateTask && (
            <button
              onClick={onCreateTask}
              className="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg text-sm font-bold transition-colors shadow-lg shadow-primary/20 shrink-0"
            >
              <span className="material-symbols-outlined text-[20px]">add</span>
              <span className="hidden sm:inline">Create Task</span>
            </button>
          )}
        </div>
      </div>

      {/* Kanban Board Area */}
      <div className="flex-1 min-h-0 overflow-x-auto overflow-y-auto overscroll-x-contain bg-background">
        <KanbanProvider onTaskMove={handleTaskMove}>
          {columns.map((column) => (
            <KanbanColumn
              key={column.id}
              column={column}
              onTaskClick={(id) => {
                const task = column.tasks.find((t) => t.id === id);
                if (task) onTaskClick(task);
              }}
              onAddTask={onCreateTask}
              selectedTaskId={selectedTaskId}
              onTaskStart={onTaskStart}
              onTaskCancelExecution={onTaskCancelExecution}
              onTaskDelete={onTaskDelete}
              onTaskViewDetails={onTaskViewDetails}
              onTaskEdit={onTaskEdit}
              onTaskNewAttempt={onTaskNewAttempt}
              onTaskRetry={onTaskRetry}
              isAllProjects={showProjectChips}
              onTaskClose={column.status === 'done' ? onTaskClose : undefined}
              onCloseAllDone={column.status === 'done' ? onCloseAllDone : undefined}
            />
          ))}
        </KanbanProvider>
      </div>
    </div>
  );
}
