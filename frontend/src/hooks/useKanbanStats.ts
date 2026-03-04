/**
 * useKanbanStats Hook - Fetch real stats for Kanban board
 *
 * Calculates:
 * - Open Tasks: non-done tasks count
 * - Sprint Progress: from active sprint (single project) or overall completion (all projects)
 * - Agent Velocity: tasks completed per hour
 * - Active Agents: per-project when a project is selected, global when "All Projects"
 *
 * Accepts tasks externally (from useKanban) to avoid duplicate API calls.
 */

import { useMemo, useState, useEffect } from 'react';
import { useGetDashboard } from '../api/generated/dashboard/dashboard';
import { useGetActiveSprint } from '../api/generated/sprints/sprints';
import { apiGet, API_PREFIX } from '../api/client';
import type { TaskDto } from '../api/generated/models';

interface KanbanStats {
  sprintProgress: {
    percentage: number;
    trend: string;
    /** Label to show — "Sprint Progress" or "Overall Progress" */
    label: string;
  };
  agentVelocity: {
    tasksPerHour: number;
  };
  activeAgents: {
    count: number;
    label: string;
  };
  openTasks: number;
}

export function useKanbanStats(
  projectId?: string,
  tasks?: TaskDto[],
): {
  stats: KanbanStats;
  loading: boolean;
} {
  // Determine if we should fetch all tasks (when projectId is 'all' or undefined)
  const isAllProjects = projectId === 'all' || !projectId;
  const apiProjectId = isAllProjects ? '' : (projectId || '');

  // Fetch dashboard stats for active agents (global)
  const { data: dashboardResponse, isLoading: dashboardLoading } = useGetDashboard({
    query: {
      staleTime: 30000, // 30 seconds
    },
  });

  // Fetch active sprint for specific project
  const { data: activeSprintResponse, isLoading: sprintLoading } = useGetActiveSprint(
    apiProjectId,
    {
      query: {
        enabled: !isAllProjects && !!apiProjectId,
        staleTime: 30000,
      },
    }
  );

  // Fetch per-project active agents count when a specific project is selected
  const [projectActiveAgentCount, setProjectActiveAgentCount] = useState(0);
  useEffect(() => {
    if (isAllProjects || !apiProjectId) {
      setProjectActiveAgentCount(0);
      return;
    }

    let cancelled = false;
    const fetchProjectAgents = async () => {
      try {
        const agents = await apiGet<Array<{ attempt_id: string }>>(
          `${API_PREFIX}/projects/${apiProjectId}/agents/active`
        );
        if (!cancelled) {
          setProjectActiveAgentCount(Array.isArray(agents) ? agents.length : 0);
        }
      } catch {
        if (!cancelled) setProjectActiveAgentCount(0);
      }
    };

    fetchProjectAgents();
    const interval = setInterval(fetchProjectAgents, 15000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [isAllProjects, apiProjectId]);

  const stats = useMemo(() => {
    // ── Active Agents ──
    // Per-project when a project is selected, global when "All Projects"
    const globalAgentsOnline = dashboardResponse?.data?.stats?.agentsOnline?.online || 0;
    const activeAgents = isAllProjects
      ? { count: globalAgentsOnline, label: 'Online' }
      : { count: projectActiveAgentCount, label: 'Working' };

    // ── Open Tasks count ──
    const taskList = tasks || [];
    const isDoneLike = (status: string) =>
      ['done', 'archived', 'cancelled', 'canceled'].includes(status.toLowerCase());
    const openTasks = taskList.filter(
      (task) => !isDoneLike(task.status)
    ).length;

    // ── Sprint Progress ──
    let sprintProgress = { percentage: 0, trend: '', label: 'Sprint Progress' };

    if (!isAllProjects && activeSprintResponse?.data) {
      // Specific project with active sprint → real sprint progress
      const sprint = activeSprintResponse.data;
      const sprintTasks = taskList.filter((task) => task.sprint_id === sprint.id);
      const completedTasks = sprintTasks.filter((task) => isDoneLike(task.status)).length;
      const totalSprintTasks = sprintTasks.length;

      if (totalSprintTasks > 0) {
        const percentage = Math.round((completedTasks / totalSprintTasks) * 100);
        const remaining = totalSprintTasks - completedTasks;
        const trend = remaining === 0 ? 'Complete' : `${remaining} left`;
        sprintProgress = { percentage, trend, label: 'Sprint Progress' };
      } else {
        sprintProgress = { percentage: 0, trend: 'No tasks in sprint', label: 'Sprint Progress' };
      }
    } else {
      // "All Projects" or no active sprint → overall completion
      const completedTasks = taskList.filter((task) => isDoneLike(task.status)).length;
      const totalTasks = taskList.length;

      if (totalTasks > 0) {
        const percentage = Math.round((completedTasks / totalTasks) * 100);
        const trend = `${completedTasks}/${totalTasks}`;
        sprintProgress = { percentage, trend, label: 'Completion' };
      } else {
        sprintProgress = { percentage: 0, trend: 'No tasks', label: 'Completion' };
      }
    }

    // ── Agent Velocity ──
    // Count tasks completed in the last 24 hours, show as tasks/day for meaningful data
    const oneDayAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);
    const recentCompletedTasks = taskList.filter((task) => {
      if (!isDoneLike(task.status)) return false;
      const updatedAt = new Date(task.updated_at);
      return updatedAt >= oneDayAgo;
    });
    const tasksPerHour = recentCompletedTasks.length;

    return {
      sprintProgress,
      agentVelocity: {
        tasksPerHour,
      },
      activeAgents,
      openTasks,
    };
  }, [dashboardResponse, activeSprintResponse, tasks, isAllProjects, projectActiveAgentCount]);

  const loading = dashboardLoading || sprintLoading;

  return {
    stats,
    loading,
  };
}
