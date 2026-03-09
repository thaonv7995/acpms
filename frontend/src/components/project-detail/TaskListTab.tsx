// TaskListTab - List view of tasks for ProjectDetail
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { closeProjectSprint, type CloseSprintResult, type SprintCarryOverMode, type SprintWithRoadmapFields } from '../../api/sprints';
import type { Requirement } from '../../api/requirements';
import type { KanbanTask } from '../../types/project';
import { logger } from '@/lib/logger';

interface TaskListTabProps {
    tasks: KanbanTask[];
    requirements: Requirement[];
    projectId: string;
    sprints: SprintWithRoadmapFields[];
    selectedSprintId: string | null;
    onSelectSprint: (sprintId: string | null) => void;
    onRefreshProject: () => Promise<void> | void;
    onAddTask?: () => void;
    onTaskClick?: (taskId: string) => void;
    onViewLogs?: (taskId: string) => void;
    onEditTask?: (taskId: string) => void;
    onDeleteTask?: (taskId: string) => void;
    onPaginationVisibilityChange?: (visible: boolean) => void;
}

const PAGE_SIZE = 10;

const statusStyles: Record<KanbanTask['status'], { bg: string; text: string; label: string }> = {
    backlog: { bg: 'bg-muted', text: 'text-muted-foreground', label: 'Backlog' },
    todo: { bg: 'bg-muted', text: 'text-muted-foreground', label: 'To Do' },
    in_progress: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', label: 'In Progress' },
    in_review: { bg: 'bg-amber-100 dark:bg-amber-500/20', text: 'text-amber-600 dark:text-amber-400', label: 'In Review' },
    blocked: { bg: 'bg-red-100 dark:bg-red-500/20', text: 'text-red-600 dark:text-red-400', label: 'Blocked' },
    done: { bg: 'bg-green-100 dark:bg-green-500/20', text: 'text-green-600 dark:text-green-400', label: 'Done' },
    archived: { bg: 'bg-muted', text: 'text-muted-foreground', label: 'Archived' },
};

const typeStyles: Record<KanbanTask['type'], { bg: string; text: string; label: string }> = {
    feature: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', label: 'Feature' },
    bug: { bg: 'bg-red-100 dark:bg-red-500/20', text: 'text-red-600 dark:text-red-400', label: 'Bug' },
    hotfix: { bg: 'bg-rose-100 dark:bg-rose-500/20', text: 'text-rose-600 dark:text-rose-400', label: 'Hotfix' },
    refactor: { bg: 'bg-orange-100 dark:bg-orange-500/20', text: 'text-orange-600 dark:text-orange-400', label: 'Refactor' },
    docs: { bg: 'bg-teal-100 dark:bg-teal-500/20', text: 'text-teal-600 dark:text-teal-400', label: 'Docs' },
    test: { bg: 'bg-indigo-100 dark:bg-indigo-500/20', text: 'text-indigo-600 dark:text-indigo-400', label: 'Test' },
    chore: { bg: 'bg-muted', text: 'text-muted-foreground', label: 'Chore' },
    spike: { bg: 'bg-amber-100 dark:bg-amber-500/20', text: 'text-amber-700 dark:text-amber-300', label: 'Spike' },
    small_task: { bg: 'bg-slate-100 dark:bg-slate-700', text: 'text-slate-700 dark:text-slate-200', label: 'Small Task' },
    deploy: { bg: 'bg-emerald-100 dark:bg-emerald-500/20', text: 'text-emerald-600 dark:text-emerald-400', label: 'Deploy' },
    init: { bg: 'bg-gray-100 dark:bg-gray-500/20', text: 'text-gray-600 dark:text-gray-400', label: 'Init' },
};

const priorityStyles: Record<KanbanTask['priority'], { bg: string; text: string; icon: string }> = {
    critical: { bg: 'bg-red-100 dark:bg-red-500/20', text: 'text-red-600 dark:text-red-400', icon: 'error' },
    high: { bg: 'bg-orange-100 dark:bg-orange-500/20', text: 'text-orange-600 dark:text-orange-400', icon: 'priority_high' },
    medium: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', icon: 'remove' },
    low: { bg: 'bg-muted', text: 'text-muted-foreground', icon: 'expand_more' },
};

interface AttemptActionConfig {
    ariaLabel: string;
    icon: string;
    pulse?: boolean;
    buttonClassName: string;
}

function getAttemptActionConfig(task: KanbanTask): AttemptActionConfig | null {
    if (task.agentWorking) {
        return {
            ariaLabel: `Open live logs for ${task.title}`,
            icon: 'smart_toy',
            pulse: true,
            buttonClassName: 'text-blue-600 hover:bg-blue-50 hover:text-blue-700 dark:text-blue-300 dark:hover:bg-blue-900/20 dark:hover:text-blue-200',
        };
    }

    if (task.status === 'in_review') {
        return {
            ariaLabel: `Open review logs for ${task.title}`,
            icon: 'rate_review',
            buttonClassName: 'text-amber-600 hover:bg-amber-50 hover:text-amber-700 dark:text-amber-300 dark:hover:bg-amber-900/20 dark:hover:text-amber-200',
        };
    }

    if (task.status === 'done') {
        return {
            ariaLabel: `Open logs for ${task.title}`,
            icon: 'terminal',
            buttonClassName: 'text-emerald-600 hover:bg-emerald-50 hover:text-emerald-700 dark:text-emerald-300 dark:hover:bg-emerald-900/20 dark:hover:text-emerald-200',
        };
    }

    return null;
}

function normalizeSprintStatus(status: string | undefined | null): string {
    if (!status) return 'planned';
    const lower = status.toLowerCase();
    if (lower === 'planning') return 'planned';
    if (lower === 'completed') return 'closed';
    return lower;
}

function formatSprintDateRange(startDate?: string | null, endDate?: string | null): string {
    if (!startDate && !endDate) return 'No date range';

    const formatDate = (value?: string | null) => {
        if (!value) return '—';
        const parsed = new Date(value);
        if (Number.isNaN(parsed.getTime())) return '—';
        return parsed.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    };

    return `${formatDate(startDate)} - ${formatDate(endDate)}`;
}

export function TaskListTab({
    tasks,
    requirements,
    projectId,
    sprints,
    selectedSprintId,
    onSelectSprint,
    onRefreshProject,
    onAddTask,
    onTaskClick,
    onViewLogs,
    onEditTask,
    onDeleteTask,
    onPaginationVisibilityChange,
}: TaskListTabProps) {
    const navigate = useNavigate();
    const [searchQuery, setSearchQuery] = useState('');
    const [statusFilter, setStatusFilter] = useState<KanbanTask['status'] | 'all'>('all');
    const [typeFilter, setTypeFilter] = useState<KanbanTask['type'] | 'all'>('all');
    const [requirementFilter, setRequirementFilter] = useState<string>('all');
    const [currentPage, setCurrentPage] = useState(1);

    const [showCloseSprintModal, setShowCloseSprintModal] = useState(false);
    const [carryOverMode, setCarryOverMode] = useState<SprintCarryOverMode>('move_to_next');
    const [nextSprintTarget, setNextSprintTarget] = useState<'existing' | 'create'>('create');
    const [selectedNextSprintId, setSelectedNextSprintId] = useState('');
    const [nextSprintName, setNextSprintName] = useState('');
    const [nextSprintGoal, setNextSprintGoal] = useState('');
    const [nextSprintStartDate, setNextSprintStartDate] = useState('');
    const [nextSprintEndDate, setNextSprintEndDate] = useState('');
    const [closeReason, setCloseReason] = useState('');
    const [closeSprintLoading, setCloseSprintLoading] = useState(false);
    const [closeSprintError, setCloseSprintError] = useState<string | null>(null);
    const [closeSprintResult, setCloseSprintResult] = useState<CloseSprintResult | null>(null);

    const sortedSprints = useMemo(() => {
        return [...sprints].sort((a, b) => {
            const aSequence = typeof a.sequence === 'number' ? a.sequence : Number.MAX_SAFE_INTEGER;
            const bSequence = typeof b.sequence === 'number' ? b.sequence : Number.MAX_SAFE_INTEGER;
            if (aSequence !== bSequence) return aSequence - bSequence;
            return new Date(a.created_at).getTime() - new Date(b.created_at).getTime();
        });
    }, [sprints]);

    const selectedSprint = useMemo(
        () => sortedSprints.find((sprint) => sprint.id === selectedSprintId) || null,
        [sortedSprints, selectedSprintId],
    );

    const activeSprint = useMemo(
        () => sortedSprints.find((sprint) => normalizeSprintStatus(sprint.status) === 'active') || null,
        [sortedSprints],
    );

    const sprintToClose = useMemo(() => {
        if (selectedSprint && normalizeSprintStatus(selectedSprint.status) === 'active') {
            return selectedSprint;
        }
        if (!selectedSprintId && activeSprint) {
            return activeSprint;
        }
        return null;
    }, [selectedSprint, selectedSprintId, activeSprint]);

    const plannedCandidateSprints = useMemo(() => {
        return sortedSprints.filter((sprint) => {
            if (!sprintToClose || sprint.id === sprintToClose.id) return false;
            const status = normalizeSprintStatus(sprint.status);
            return status === 'planned';
        });
    }, [sortedSprints, sprintToClose]);

    const nextSprintSequence = useMemo(() => {
        const maxSequence = sortedSprints.reduce((acc, sprint) => {
            if (typeof sprint.sequence === 'number') {
                return Math.max(acc, sprint.sequence);
            }
            return acc;
        }, 0);
        return maxSequence + 1;
    }, [sortedSprints]);

    const requirementMap = useMemo(() => {
        const map = new Map<string, Requirement>();
        requirements.forEach((requirement) => {
            map.set(requirement.id, requirement);
        });
        return map;
    }, [requirements]);

    const requirementFilterOptions = useMemo(() => {
        const linkedRequirementIds = new Set<string>();
        tasks.forEach((task) => {
            if (task.requirement_id) {
                linkedRequirementIds.add(task.requirement_id);
            }
        });

        return requirements
            .filter((requirement) => linkedRequirementIds.has(requirement.id))
            .sort((a, b) => a.title.localeCompare(b.title));
    }, [requirements, tasks]);

    useEffect(() => {
        if (!showCloseSprintModal) return;

        setCarryOverMode('move_to_next');
        setCloseSprintError(null);
        setCloseReason('');

        if (plannedCandidateSprints.length > 0) {
            setNextSprintTarget('existing');
            setSelectedNextSprintId(plannedCandidateSprints[0].id);
        } else {
            setNextSprintTarget('create');
            setSelectedNextSprintId('');
        }

        setNextSprintName(`Sprint ${nextSprintSequence}`);
        setNextSprintGoal('');
        setNextSprintStartDate('');
        setNextSprintEndDate('');
    }, [showCloseSprintModal, plannedCandidateSprints, nextSprintSequence]);

    useEffect(() => {
        setCurrentPage(1);
    }, [searchQuery, statusFilter, typeFilter, requirementFilter, selectedSprintId]);

    const handleTaskClick = (taskId: string) => {
        if (onTaskClick) {
            onTaskClick(taskId);
        } else {
            navigate(`/projects/${projectId}/task/${taskId}`);
        }
    };

    const handleCloseSprint = async () => {
        if (!sprintToClose) {
            setCloseSprintError('No active sprint selected to complete.');
            return;
        }

        if (
            carryOverMode === 'move_to_next'
            && nextSprintTarget === 'existing'
            && !selectedNextSprintId
        ) {
            setCloseSprintError('Please select a planned sprint to move unfinished tasks.');
            return;
        }

        if (
            carryOverMode === 'move_to_next'
            && nextSprintTarget === 'create'
            && !nextSprintName.trim()
        ) {
            setCloseSprintError('Sprint name is required when creating next sprint.');
            return;
        }

        if (
            carryOverMode === 'move_to_next'
            && nextSprintTarget === 'create'
            && nextSprintStartDate
            && nextSprintEndDate
            && new Date(nextSprintStartDate) > new Date(nextSprintEndDate)
        ) {
            setCloseSprintError('Next sprint end date must be after start date.');
            return;
        }

        setCloseSprintLoading(true);
        setCloseSprintError(null);

        try {
            const payload: {
                carry_over_mode: SprintCarryOverMode;
                next_sprint_id?: string;
                create_next_sprint?: {
                    name?: string;
                    goal?: string;
                    start_date?: string;
                    end_date?: string;
                };
                reason?: string;
            } = {
                carry_over_mode: carryOverMode,
                reason: closeReason.trim() || undefined,
            };

            if (carryOverMode === 'move_to_next') {
                if (nextSprintTarget === 'existing') {
                    payload.next_sprint_id = selectedNextSprintId;
                } else {
                    payload.create_next_sprint = {
                        name: nextSprintName.trim() || undefined,
                        goal: nextSprintGoal.trim() || undefined,
                        start_date: nextSprintStartDate ? new Date(nextSprintStartDate).toISOString() : undefined,
                        end_date: nextSprintEndDate ? new Date(nextSprintEndDate).toISOString() : undefined,
                    };
                }
            }

            const result = await closeProjectSprint(projectId, sprintToClose.id, payload);
            setCloseSprintResult(result);
            await onRefreshProject();

            if (result.movedToSprintId) {
                onSelectSprint(result.movedToSprintId);
            } else {
                onSelectSprint(null);
            }

            setShowCloseSprintModal(false);
        } catch (error) {
            logger.error('Failed to close sprint:', error);
            setCloseSprintError(error instanceof Error ? error.message : 'Failed to complete sprint');
        } finally {
            setCloseSprintLoading(false);
        }
    };

    // Filter tasks
    const filteredTasks = useMemo(() => {
        return tasks.filter((task) => {
            const matchesSearch = !searchQuery
                || task.title.toLowerCase().includes(searchQuery.toLowerCase())
                || task.description?.toLowerCase().includes(searchQuery.toLowerCase());
            const matchesStatus = statusFilter === 'all' || task.status === statusFilter;
            const matchesType = typeFilter === 'all' || task.type === typeFilter;
            const matchesRequirement = requirementFilter === 'all'
                || (requirementFilter === '__none__'
                    ? !task.requirement_id
                    : task.requirement_id === requirementFilter);
            return matchesSearch && matchesStatus && matchesType && matchesRequirement;
        });
    }, [tasks, searchQuery, statusFilter, typeFilter, requirementFilter]);

    // Sort: in_progress first, then review, then backlog/todo, then closed states
    const sortedTasks = useMemo(() => {
        return [...filteredTasks].sort((a, b) => {
            const statusOrder: Record<string, number> = {
                in_progress: 0,
                in_review: 1,
                backlog: 2,
                todo: 3,
                done: 4,
                archived: 5,
            };
            return (statusOrder[a.status] ?? 6) - (statusOrder[b.status] ?? 6);
        });
    }, [filteredTasks]);

    const totalPages = Math.max(1, Math.ceil(sortedTasks.length / PAGE_SIZE));
    const pageStart = sortedTasks.length === 0 ? 0 : (currentPage - 1) * PAGE_SIZE + 1;
    const pageEnd = Math.min(currentPage * PAGE_SIZE, sortedTasks.length);
    const shownRangeLabel = sortedTasks.length === 0 ? '0' : `${pageStart}-${pageEnd}`;
    const showPaginationControls = sortedTasks.length > 0;
    const paginatedTasks = useMemo(() => {
        const startIdx = (currentPage - 1) * PAGE_SIZE;
        return sortedTasks.slice(startIdx, startIdx + PAGE_SIZE);
    }, [sortedTasks, currentPage]);

    useEffect(() => {
        setCurrentPage((prev) => (prev > totalPages ? totalPages : prev));
    }, [totalPages]);

    useEffect(() => {
        onPaginationVisibilityChange?.(showPaginationControls);
        return () => onPaginationVisibilityChange?.(false);
    }, [onPaginationVisibilityChange, showPaginationControls]);

    return (
        <>
            <div className="flex flex-col gap-4">
                {closeSprintResult && (
                    <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-600 dark:text-emerald-400">
                        Sprint completed. Moved {closeSprintResult.movedTaskCount} unfinished task(s)
                        {closeSprintResult.movedToSprintId ? ' to next sprint.' : '.'}
                    </div>
                )}

                {/* Toolbar */}
                <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
                    <div className="flex flex-wrap gap-2 items-center">
                        {/* Search */}
                        <div className="relative">
                            <span className="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground text-[18px]">search</span>
                            <input
                                type="text"
                                placeholder="Search tasks..."
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                                className="pl-9 pr-4 py-2 w-64 bg-card border border-border rounded-lg text-sm text-card-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary"
                            />
                        </div>

                        {/* Status Filter */}
                        <select
                            value={statusFilter}
                            onChange={(e) => setStatusFilter(e.target.value as KanbanTask['status'] | 'all')}
                            className="px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                        >
                            <option value="all">All Status</option>
                            <option value="backlog">Backlog</option>
                            <option value="todo">To Do</option>
                            <option value="in_progress">In Progress</option>
                            <option value="in_review">In Review</option>
                            <option value="blocked">Blocked</option>
                            <option value="done">Done</option>
                            <option value="archived">Archived</option>
                        </select>

                        {/* Type Filter */}
                        <select
                            value={typeFilter}
                            onChange={(e) => setTypeFilter(e.target.value as KanbanTask['type'] | 'all')}
                            className="px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                        >
                            <option value="all">All Types</option>
                            <option value="feature">Feature</option>
                            <option value="bug">Bug</option>
                            <option value="hotfix">Hotfix</option>
                            <option value="refactor">Refactor</option>
                            <option value="docs">Docs</option>
                            <option value="test">Test</option>
                            <option value="chore">Chore</option>
                            <option value="spike">Spike</option>
                            <option value="small_task">Small Task</option>
                            <option value="deploy">Deploy</option>
                        </select>

                        {/* Requirement Filter */}
                        <select
                            value={requirementFilter}
                            onChange={(e) => setRequirementFilter(e.target.value)}
                            className="max-w-[280px] px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                        >
                            <option value="all">All Requirements</option>
                            <option value="__none__">No Requirement Link</option>
                            {requirementFilterOptions.map((requirement) => (
                                <option key={requirement.id} value={requirement.id}>
                                    {requirement.title}
                                </option>
                            ))}
                        </select>
                    </div>

                    <div className="flex items-center gap-2">
                        <button
                            onClick={() => setShowCloseSprintModal(true)}
                            disabled={!sprintToClose}
                            className="flex items-center gap-2 px-4 py-2 bg-amber-500 hover:bg-amber-500/90 text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                            title={sprintToClose ? `Complete ${sprintToClose.name}` : 'No active sprint selected'}
                        >
                            <span className="material-symbols-outlined text-[18px]">flag</span>
                            Complete Sprint
                        </button>

                        {/* Add Task Button */}
                        {onAddTask && (
                            <button
                                onClick={onAddTask}
                                className="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg text-sm font-medium transition-colors"
                            >
                                <span className="material-symbols-outlined text-[18px]">add</span>
                                Add Task
                            </button>
                        )}
                    </div>
                </div>

                {/* Task Count */}
                <div className="text-sm text-muted-foreground">
                    Showing {shownRangeLabel} of {sortedTasks.length} filtered tasks ({tasks.length} total)
                </div>

                {/* Task List */}
                <div className="bg-card border border-border rounded-xl overflow-hidden">
                    {/* Table Header */}
                    <div className="grid grid-cols-12 gap-4 px-4 py-3 bg-muted/50 border-b border-border text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        <div className="col-span-1">Status</div>
                        <div className="col-span-4">Title</div>
                        <div className="col-span-2">Type</div>
                        <div className="col-span-2">Priority</div>
                        <div className="col-span-3 text-right">Quick Actions</div>
                    </div>

                    {/* Task Rows */}
                    {sortedTasks.length === 0 ? (
                        <div className="px-4 py-8 text-center text-muted-foreground">
                            <span className="material-symbols-outlined text-4xl mb-2 block">inbox</span>
                            No tasks found
                        </div>
                    ) : (
                        paginatedTasks.map((task) => {
                            const status = statusStyles[task.status];
                            const type = typeStyles[task.type];
                            const priority = priorityStyles[task.priority];
                            const isWorking = !!task.agentWorking;
                            const canEditTask =
                                task.status === 'backlog'
                                || task.status === 'todo'
                                || task.status === 'in_review';
                            const linkedRequirement = task.requirement_id
                                ? requirementMap.get(task.requirement_id)
                                : null;
                            const attemptAction = getAttemptActionConfig(task);

                            return (
                                <div
                                    key={task.id}
                                    onClick={() => handleTaskClick(task.id)}
                                    className={`grid grid-cols-12 gap-4 px-4 py-3 border-b border-border hover:bg-muted/50 cursor-pointer transition-colors ${
                                        isWorking ? 'bg-blue-50/50 dark:bg-blue-500/10' : ''
                                        }`}
                                >
                                    {/* Status */}
                                    <div className="col-span-1 flex items-center">
                                        <span className={`px-2 py-1 rounded text-[10px] font-bold ${status.bg} ${status.text}`}>
                                            {task.status === 'in_progress' ? 'WIP' : task.status === 'in_review' ? 'REV' : task.status === 'done' ? '✓' : '○'}
                                        </span>
                                    </div>

                                    {/* Title */}
                                    <div className="col-span-4 flex flex-col justify-center min-w-0">
                                        <span className={`font-medium text-sm text-card-foreground truncate ${task.status === 'done' ? 'line-through opacity-60' : ''}`}>
                                            {task.title}
                                        </span>
                                        {task.description && (
                                            <span className="text-xs text-muted-foreground truncate">
                                                {task.description}
                                            </span>
                                        )}
                                        {linkedRequirement && (
                                            <span className="text-[11px] text-primary/90 truncate mt-0.5">
                                                Requirement: {linkedRequirement.title}
                                            </span>
                                        )}
                                    </div>

                                    {/* Type */}
                                    <div className="col-span-2 flex items-center">
                                        <span className={`px-2 py-1 rounded text-[10px] font-bold uppercase ${type.bg} ${type.text}`}>
                                            {type.label}
                                        </span>
                                    </div>

                                    {/* Priority */}
                                    <div className="col-span-2 flex items-center gap-1">
                                        <span className={`material-symbols-outlined text-[16px] ${priority.text}`}>
                                            {priority.icon}
                                        </span>
                                        <span className={`text-xs font-medium ${priority.text}`}>
                                            {task.priority}
                                        </span>
                                    </div>

                                    {/* Quick Actions */}
                                    <div className="col-span-3 flex items-center justify-end">
                                        <div className="flex min-w-0 items-center justify-end gap-1">
                                            {attemptAction && (
                                                <button
                                                    type="button"
                                                    onClick={(e) => {
                                                        e.stopPropagation();
                                                        onViewLogs?.(task.id);
                                                    }}
                                                    className={`flex h-9 w-9 items-center justify-center rounded-xl transition-colors ${attemptAction.buttonClassName}`}
                                                    title={attemptAction.ariaLabel}
                                                    aria-label={attemptAction.ariaLabel}
                                                >
                                                    <span className={`material-symbols-outlined text-[18px] ${attemptAction.pulse ? 'animate-pulse' : ''}`}>
                                                        {attemptAction.icon}
                                                    </span>
                                                </button>
                                            )}
                                            {canEditTask && onEditTask && (
                                                <button
                                                    type="button"
                                                    onClick={(e) => {
                                                        e.stopPropagation();
                                                        onEditTask(task.id);
                                                    }}
                                                    className="flex h-9 w-9 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:bg-muted hover:text-card-foreground"
                                                    title="Edit task"
                                                    aria-label={`Edit ${task.title}`}
                                                >
                                                    <span className="material-symbols-outlined text-[18px]">edit</span>
                                                </button>
                                            )}
                                            {onDeleteTask && (
                                                <button
                                                    type="button"
                                                    onClick={(e) => {
                                                        e.stopPropagation();
                                                        onDeleteTask(task.id);
                                                    }}
                                                    className="flex h-9 w-9 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:bg-red-500/10 hover:text-red-600 dark:hover:text-red-300"
                                                    title="Delete task"
                                                    aria-label={`Delete ${task.title}`}
                                                >
                                                    <span className="material-symbols-outlined text-[18px]">delete</span>
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            );
                        })
                    )}
                </div>

                {showPaginationControls && (
                    <div className="flex items-center justify-between">
                        <p className="text-xs text-muted-foreground">
                            Page {currentPage} / {totalPages}
                        </p>
                        <div className="flex items-center gap-2">
                            <button
                                onClick={() => setCurrentPage((prev) => Math.max(1, prev - 1))}
                                disabled={currentPage <= 1}
                                className="px-3 py-1.5 text-xs rounded border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                Previous
                            </button>
                            <button
                                onClick={() => setCurrentPage((prev) => Math.min(totalPages, prev + 1))}
                                disabled={currentPage >= totalPages}
                                className="px-3 py-1.5 text-xs rounded border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                Next
                            </button>
                        </div>
                    </div>
                )}
            </div>

            {/* Complete Sprint Modal */}
            {showCloseSprintModal && (
                <div className="fixed inset-0 z-[100] flex items-center justify-center p-4">
                    <div
                        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
                        onClick={() => !closeSprintLoading && setShowCloseSprintModal(false)}
                    />
                    <div className="relative w-full max-w-2xl bg-card border border-border rounded-xl shadow-2xl p-6 space-y-4 max-h-[90vh] overflow-y-auto">
                        <div className="flex items-start justify-between gap-3">
                            <div>
                                <h4 className="text-lg font-bold text-card-foreground">Complete Sprint</h4>
                                <p className="text-sm text-muted-foreground">
                                    {sprintToClose
                                        ? `Complete ${sprintToClose.name} and define what happens to unfinished tasks.`
                                        : 'No active sprint available to complete.'}
                                </p>
                            </div>
                            <button
                                onClick={() => setShowCloseSprintModal(false)}
                                disabled={closeSprintLoading}
                                className="text-muted-foreground hover:text-card-foreground"
                            >
                                <span className="material-symbols-outlined">close</span>
                            </button>
                        </div>

                        {sprintToClose ? (
                            <>
                                <div className="rounded-lg border border-border bg-muted/40 p-3">
                                    <p className="text-sm font-medium text-card-foreground">Current Sprint</p>
                                    <p className="text-xs text-muted-foreground mt-1">
                                        {formatSprintDateRange(sprintToClose.start_date, sprintToClose.end_date)}
                                    </p>
                                </div>

                                <div className="space-y-2">
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Carry-over strategy</label>
                                    <div className="space-y-2">
                                        <label className="flex items-start gap-2 p-3 rounded-lg border border-border hover:bg-muted/40 cursor-pointer">
                                            <input
                                                type="radio"
                                                name="carry-over-mode"
                                                checked={carryOverMode === 'move_to_next'}
                                                onChange={() => setCarryOverMode('move_to_next')}
                                                className="mt-0.5"
                                            />
                                            <div>
                                                <p className="text-sm font-medium text-card-foreground">Move unfinished tasks to next sprint</p>
                                                <p className="text-xs text-muted-foreground">Recommended for continuous delivery between sprints.</p>
                                            </div>
                                        </label>

                                        <label className="flex items-start gap-2 p-3 rounded-lg border border-border hover:bg-muted/40 cursor-pointer">
                                            <input
                                                type="radio"
                                                name="carry-over-mode"
                                                checked={carryOverMode === 'move_to_backlog'}
                                                onChange={() => setCarryOverMode('move_to_backlog')}
                                                className="mt-0.5"
                                            />
                                            <div>
                                                <p className="text-sm font-medium text-card-foreground">Move unfinished tasks to backlog</p>
                                                <p className="text-xs text-muted-foreground">Sprint closes cleanly, remaining work returns to backlog.</p>
                                            </div>
                                        </label>

                                        <label className="flex items-start gap-2 p-3 rounded-lg border border-border hover:bg-muted/40 cursor-pointer">
                                            <input
                                                type="radio"
                                                name="carry-over-mode"
                                                checked={carryOverMode === 'keep_in_closed'}
                                                onChange={() => setCarryOverMode('keep_in_closed')}
                                                className="mt-0.5"
                                            />
                                            <div>
                                                <p className="text-sm font-medium text-card-foreground">Keep unfinished tasks in closed sprint</p>
                                                <p className="text-xs text-muted-foreground">Preserve exact sprint snapshot for historical reporting.</p>
                                            </div>
                                        </label>
                                    </div>
                                </div>

                                {carryOverMode === 'move_to_next' && (
                                    <div className="space-y-3 border-t border-border pt-3">
                                        <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Next sprint target</label>

                                        {plannedCandidateSprints.length > 0 && (
                                            <div className="flex items-center gap-4 text-sm">
                                                <label className="flex items-center gap-2">
                                                    <input
                                                        type="radio"
                                                        checked={nextSprintTarget === 'existing'}
                                                        onChange={() => setNextSprintTarget('existing')}
                                                    />
                                                    Use planned sprint
                                                </label>
                                                <label className="flex items-center gap-2">
                                                    <input
                                                        type="radio"
                                                        checked={nextSprintTarget === 'create'}
                                                        onChange={() => setNextSprintTarget('create')}
                                                    />
                                                    Create new sprint
                                                </label>
                                            </div>
                                        )}

                                        {nextSprintTarget === 'existing' && plannedCandidateSprints.length > 0 ? (
                                            <select
                                                value={selectedNextSprintId}
                                                onChange={(event) => setSelectedNextSprintId(event.target.value)}
                                                className="w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                                            >
                                                {plannedCandidateSprints.map((sprint) => (
                                                    <option key={sprint.id} value={sprint.id}>
                                                        #{typeof sprint.sequence === 'number' ? sprint.sequence : '—'} {sprint.name}
                                                    </option>
                                                ))}
                                            </select>
                                        ) : (
                                            <div className="space-y-3">
                                                <input
                                                    value={nextSprintName}
                                                    onChange={(event) => setNextSprintName(event.target.value)}
                                                    placeholder={`Sprint ${nextSprintSequence}`}
                                                    className="w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                                                />
                                                <textarea
                                                    value={nextSprintGoal}
                                                    onChange={(event) => setNextSprintGoal(event.target.value)}
                                                    rows={2}
                                                    placeholder="Sprint goal"
                                                    className="w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                                                />
                                                <div className="grid grid-cols-2 gap-3">
                                                    <input
                                                        type="datetime-local"
                                                        value={nextSprintStartDate}
                                                        onChange={(event) => setNextSprintStartDate(event.target.value)}
                                                        className="px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                                                    />
                                                    <input
                                                        type="datetime-local"
                                                        value={nextSprintEndDate}
                                                        onChange={(event) => setNextSprintEndDate(event.target.value)}
                                                        className="px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                                                    />
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                )}

                                <div>
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Reason (optional)</label>
                                    <textarea
                                        value={closeReason}
                                        onChange={(event) => setCloseReason(event.target.value)}
                                        rows={2}
                                        placeholder="Sprint completion note"
                                        className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                                    />
                                </div>

                                {closeSprintError && (
                                    <p className="text-sm text-red-500 dark:text-red-400">{closeSprintError}</p>
                                )}

                                <div className="flex justify-end gap-2 pt-2 border-t border-border">
                                    <button
                                        onClick={() => setShowCloseSprintModal(false)}
                                        disabled={closeSprintLoading}
                                        className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
                                    >
                                        Cancel
                                    </button>
                                    <button
                                        onClick={handleCloseSprint}
                                        disabled={closeSprintLoading}
                                        className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-500/90 text-white text-sm font-semibold transition-colors disabled:opacity-60"
                                    >
                                        {closeSprintLoading ? 'Completing...' : 'Complete Sprint'}
                                    </button>
                                </div>
                            </>
                        ) : (
                            <p className="text-sm text-muted-foreground">Select an active sprint to complete.</p>
                        )}
                    </div>
                </div>
            )}
        </>
    );
}
