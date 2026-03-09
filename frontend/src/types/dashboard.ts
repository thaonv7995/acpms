// Dashboard Types
import type { ProjectLifecycleStatus } from './repository';

export interface DashboardStats {
    activeProjects: {
        count: number;
        trend: string;
    };
    agentsOnline: {
        online: number;
        total: number;
    };
    systemLoad: {
        percentage: number;
        status: 'low' | 'medium' | 'high';
    };
    pendingPRs: {
        count: number;
        requiresReview: boolean;
    };
}

export interface DashboardProject {
    id: string;
    name: string;
    subtitle: string;
    status: ProjectLifecycleStatus;
    progress: number;
    agents: {
        id: string;
        initial: string;
        color: string;
    }[];
}

export interface AgentLogEntry {
    id: string;
    timestamp: string;
    agentName: string;
    agentColor: string;
    message: string;
    highlight?: string;
}

export interface HumanTask {
    id: string;
    type: 'blocker' | 'approval' | 'qa' | 'review';
    title: string;
    description: string;
    createdAt: string;
    assignee?: {
        id: string;
        avatar?: string;
    };
}

export interface DashboardData {
    stats: DashboardStats;
    projects: DashboardProject[];
    agentLogs: AgentLogEntry[];
    humanTasks: HumanTask[];
}
