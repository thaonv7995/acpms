// Hook for Project Detail page with tabs - Real API Integration
import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { getProject } from '../api/projects';
import { getTasks } from '../api/tasks';
import { getRequirements, type Requirement } from '../api/requirements';
import { listProjectSprints } from '../api/generated/sprints/sprints';
import type { SprintDto } from '../api/generated/models';
import type { Task } from '../shared/types';
import type {
    ProjectDetail,
    KanbanColumn,
    KanbanTask,
} from '../types/project';
import type { ProjectWithRepositoryContext } from '../types/repository';
import { logger } from '@/lib/logger';

export type ProjectTab =
    | 'summary'
    | 'kanban'
    | 'requirements'
    | 'architecture'
    | 'deployments'
    | 'settings';

interface UseProjectDetailResult {
    project: ProjectDetail | null;
    rawProject: ProjectWithRepositoryContext | null;
    kanbanColumns: KanbanColumn[];
    tasks: KanbanTask[]; // Flat array of tasks for list view
    rawTasks: Task[]; // Raw tasks with requirement_id for linked tasks display
    requirements: Requirement[];
    taskStats: { total: number; byType: Record<string, number>; byStatus: Record<string, number> };
    activeTab: ProjectTab;
    setActiveTab: (tab: ProjectTab) => void;
    loading: boolean;
    error: string | null;
    refetch: () => void;
    // Sprint support
    sprints: SprintDto[];
    selectedSprintId: string | null;
    setSelectedSprintId: (id: string | null) => void;
    activeSprint: SprintDto | null;
}

// Transform backend Task to KanbanTask
function transformToKanbanTask(task: Task): KanbanTask {
    // Map task_type to KanbanTask type
    const typeMap: Record<string, KanbanTask['type']> = {
        feature: 'feature',
        bug: 'bug',
        hotfix: 'hotfix',
        refactor: 'refactor',
        docs: 'docs',
        test: 'test',
        chore: 'chore',
        spike: 'spike',
        small_task: 'small_task',
        deploy: 'deploy',
        init: 'chore',
    };

    // Map task status to KanbanTask status
    // API returns PascalCase (Todo, InProgress, InReview, Done)
    const statusMap: Record<string, KanbanTask['status']> = {
        // PascalCase from API
        Todo: 'todo',
        InProgress: 'in_progress',
        InReview: 'in_review',  // Map to dedicated in_review status
        Blocked: 'todo',
        Done: 'done',
        Archived: 'done',
        Cancelled: 'done',
        Canceled: 'done',
        // Lowercase fallbacks
        todo: 'todo',
        in_progress: 'in_progress',
        in_review: 'in_review',
        blocked: 'todo',
        done: 'done',
        archived: 'done',
        cancelled: 'done',
        canceled: 'done',
    };

    // Get priority from metadata
    const priority = (task.metadata?.priority as KanbanTask['priority']) || 'medium';

    return {
        id: task.id,
        title: task.title,
        description: task.description,
        type: typeMap[task.task_type] || 'feature',
        status: statusMap[task.status] || 'todo',
        priority,
        progress: task.metadata?.progress as number | undefined,
        assignee: task.assigned_to ? {
            id: task.assigned_to,
            initial: 'U', // TODO: Get user initial from API
            color: 'bg-blue-500',
        } : undefined,
        agentWorking: task.status === 'in_progress' && task.metadata?.agent_working ? {
            name: (task.metadata.agent_name as string) || 'Agent',
            progress: (task.metadata.agent_progress as number) || 0,
        } : undefined,
        hasIssue: task.metadata?.has_issue === true,
        createdAt: task.created_at,
    };
}

// Group tasks into Kanban columns
function groupTasksIntoColumns(tasks: Task[]): KanbanColumn[] {
    const columns: KanbanColumn[] = [
        { id: 'todo', title: 'To Do', status: 'todo', color: 'slate', tasks: [] },
        { id: 'in_progress', title: 'In Progress', status: 'in_progress', color: 'blue', tasks: [] },
        { id: 'in_review', title: 'In Review', status: 'in_review', color: 'yellow', tasks: [] },
        { id: 'done', title: 'Done', status: 'done', color: 'green', tasks: [] },
    ];

    // Filter out init tasks
    const nonInitTasks = tasks.filter(t => t.task_type !== 'init');

    nonInitTasks.forEach(task => {
        const kanbanTask = transformToKanbanTask(task);
        const column = columns.find(c => c.status === kanbanTask.status);
        if (column) {
            column.tasks.push(kanbanTask);
        }
    });

    return columns;
}

// Transform Project to ProjectDetail
function transformToProjectDetail(
    project: ProjectWithRepositoryContext,
    tasks: Task[]
): ProjectDetail {
    // Calculate stats
    const activeAgents = tasks.filter(t => t.status === 'in_progress').length;
    const pendingReview = tasks.filter(t => t.status === 'in_review').length;
    const criticalBugs = tasks.filter(t =>
        t.task_type === 'bug' &&
        (t.metadata?.priority === 'critical' || t.metadata?.priority === 'high')
    ).length;

    // Build status approximation
    const sprintTasks = tasks.filter(t => Boolean(t.sprint_id));
    const doneTasks = sprintTasks.filter(t =>
        ['done', 'archived', 'cancelled', 'canceled'].includes(t.status.toLowerCase())
    ).length;
    const totalTasks = sprintTasks.length;
    const buildStatus = totalTasks > 0 ? Math.round((doneTasks / totalTasks) * 100) : 0;

    return {
        id: project.id,
        name: project.name,
        repositoryUrl: project.repository_url || 'Not configured',
        branch: 'main', // TODO: Get from GitLab integration
        status: 'active',
        lastDeploy: 'Never', // TODO: Get from deployments API when available
        stats: {
            activeAgents,
            pendingReview,
            criticalBugs,
            buildStatus,
        },
    };
}

export function useProjectDetail(projectId: string | undefined): UseProjectDetailResult {
    const [project, setProject] = useState<ProjectDetail | null>(null);
    const [rawProject, setRawProject] = useState<ProjectWithRepositoryContext | null>(null);
    const [rawTasks, setRawTasks] = useState<Task[]>([]);
    const [requirements, setRequirements] = useState<Requirement[]>([]);
    const [sprints, setSprints] = useState<SprintDto[]>([]);
    const [selectedSprintId, setSelectedSprintId] = useState<string | null>(null);
    const [activeTab, setActiveTab] = useState<ProjectTab>('summary');
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const hasAutoSelectedSprintRef = useRef(false);

    const normalizeSprintStatus = useCallback((status: string | undefined | null): string => {
        if (!status) return '';
        const lower = status.toLowerCase();
        if (lower === 'planning') return 'planned';
        if (lower === 'completed') return 'closed';
        return lower;
    }, []);

    // Get active sprint
    const activeSprint = useMemo(
        () => sprints.find(s => normalizeSprintStatus(s.status) === 'active') || null,
        [sprints, normalizeSprintStatus],
    );

    // Filter tasks by selected sprint
    const filteredTasks = useMemo(() => {
        if (selectedSprintId === null) {
            return rawTasks; // Show all tasks when no sprint selected
        }

        return rawTasks.filter(t => t.sprint_id === selectedSprintId);
    }, [rawTasks, selectedSprintId]);

    // Memoize kanban columns from filtered tasks
    const kanbanColumns = useMemo(() => groupTasksIntoColumns(filteredTasks), [filteredTasks]);

    // Memoize flat task list for list view (excludes init tasks)
    const tasks = useMemo(() =>
        filteredTasks.filter(t => t.task_type !== 'init').map(transformToKanbanTask),
        [filteredTasks]
    );

    // Memoize task statistics (filtered by sprint)
    const taskStats = useMemo(() => {
        const nonInitTasks = filteredTasks.filter(t => t.task_type !== 'init');
        const byType: Record<string, number> = {};
        const byStatus: Record<string, number> = {};

        nonInitTasks.forEach(task => {
            byType[task.task_type] = (byType[task.task_type] || 0) + 1;
            const status = task.status.toLowerCase().replace(/([a-z])([A-Z])/g, '$1_$2').toLowerCase();
            byStatus[status] = (byStatus[status] || 0) + 1;
        });

        return { total: nonInitTasks.length, byType, byStatus };
    }, [filteredTasks]);

    const fetchData = useCallback(async () => {
        if (!projectId) {
            setError('Project ID is required');
            setLoading(false);
            return;
        }

        setLoading(true);
        setError(null);

        try {
            // Fetch project, tasks, requirements, and sprints in parallel
            const [projectData, tasksData, requirementsData, sprintsResponse] = await Promise.all([
                getProject(projectId),
                getTasks(projectId),
                getRequirements(projectId).catch(() => []), // Requirements may not exist yet
                listProjectSprints(projectId).catch(() => ({ data: [] })), // Sprints may not exist yet
            ]);

            const sprintsData = sprintsResponse?.data || [];

            setRawProject(projectData);
            setRawTasks(tasksData);
            setRequirements(requirementsData);
            setSprints(sprintsData);
            setProject(transformToProjectDetail(projectData, tasksData));

            // Auto-select sprint only on first load (avoids double fetch when setSelectedSprintId triggers effect)
            if (!hasAutoSelectedSprintRef.current && sprintsData.length > 0) {
                hasAutoSelectedSprintRef.current = true;
                const now = new Date();
                const active = sprintsData.find((s: SprintDto) => normalizeSprintStatus(s.status) === 'active');
                if (active) {
                    setSelectedSprintId(active.id);
                } else {
                    const currentSprint = sprintsData.find((s: SprintDto) => {
                        if (!s.start_date) return false;
                        const start = new Date(s.start_date);
                        const end = s.end_date ? new Date(s.end_date) : null;
                        return now >= start && (end ? now <= end : true);
                    });
                    if (currentSprint) {
                        setSelectedSprintId(currentSprint.id);
                    } else {
                        const sorted = [...sprintsData]
                            .filter((s: SprintDto) => s.start_date)
                            .sort((a: SprintDto, b: SprintDto) =>
                                new Date(b.start_date!).getTime() - new Date(a.start_date!).getTime()
                            );
                        if (sorted.length > 0) {
                            setSelectedSprintId(sorted[0].id);
                        }
                    }
                }
            }
        } catch (err) {
            logger.error('Failed to load project detail:', err);
            setError(err instanceof Error ? err.message : 'Failed to load project');
        } finally {
            setLoading(false);
        }
    }, [projectId, normalizeSprintStatus]);

    // Reset auto-select ref when project changes (e.g. navigation)
    useEffect(() => {
        hasAutoSelectedSprintRef.current = false;
    }, [projectId]);

    useEffect(() => {
        fetchData();
    }, [fetchData]);

    return {
        project,
        rawProject,
        kanbanColumns,
        tasks,
        rawTasks,
        requirements,
        taskStats,
        activeTab,
        setActiveTab,
        loading,
        error,
        refetch: fetchData,
        // Sprint support
        sprints,
        selectedSprintId,
        setSelectedSprintId,
        activeSprint,
    };
}
