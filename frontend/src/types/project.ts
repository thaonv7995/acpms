// Project Types

export interface ProjectListItem {
    id: string;
    name: string;
    description?: string;
    icon: string;
    iconColor: 'orange' | 'blue' | 'emerald' | 'purple' | 'primary';
    techStack: string[];
    status: 'agent_reviewing' | 'active_coding' | 'deploying' | 'completed' | 'paused';
    statusLabel: string;
    statusColor: 'yellow' | 'blue' | 'emerald' | 'green' | 'slate';
    progress: number;
    agentIcon: string;
    lastActivity: string;
    agentCount: number;
}

export interface ProjectDetail {
    id: string;
    name: string;
    repositoryUrl: string;
    branch: string;
    status: 'active' | 'paused' | 'archived';
    lastDeploy: string;
    stats: {
        activeAgents: number;
        pendingReview: number;
        criticalBugs: number;
        buildStatus: number; // percentage
    };
}

export interface KanbanTask {
    id: string;
    title: string;
    description?: string;
    metadata?: Record<string, unknown>;
    requirement_id?: string;
    type: 'feature' | 'bug' | 'hotfix' | 'refactor' | 'docs' | 'test' | 'chore' | 'spike' | 'small_task' | 'deploy' | 'init';
    status: 'todo' | 'in_progress' | 'in_review' | 'done' | 'archived';
    priority: 'low' | 'medium' | 'high' | 'critical';
    progress?: number;
    assignee?: {
        id: string;
        initial: string;
        color: string;
    };
    agentWorking?: {
        name: string;
        progress: number;
    };
    attachments?: number;
    /** Task has blocking issues (e.g. preflight failed, references missing). Show warning icon on kanban. */
    hasIssue?: boolean;
    latestAttemptId?: string;
    projectId?: string;
    projectName?: string;
    createdAt: string;
    attemptCount?: number;
}

export interface KanbanColumn {
    id: string;
    title: string;
    status: KanbanTask['status'];
    color: string;
    tasks: KanbanTask[];
}

export type RequirementStatus = 'draft' | 'reviewing' | 'in_review' | 'approved' | 'rejected' | 'implemented';
export type RequirementPriority = 'low' | 'medium' | 'high' | 'critical';
export type RequirementType = 'functional' | 'technical' | 'non_functional';

export interface Requirement {
    id: string;
    project_id: string;
    title: string;
    content: string;
    description?: string; // Optional field for backward compatibility
    type: RequirementType;
    status: RequirementStatus;
    priority: RequirementPriority;
    metadata?: Record<string, any>;
    created_by: string;
    created_at: string;
    updated_at: string;
}

export interface CreateRequirementRequest {
    project_id: string;
    title: string;
    content: string;
    priority?: RequirementPriority;
    metadata?: Record<string, any>;
}

export interface UpdateRequirementRequest {
    title?: string;
    content?: string;
    status?: RequirementStatus;
    priority?: RequirementPriority;
    metadata?: Record<string, any>;
}

export interface InfrastructureService {
    id: string;
    name: string;
    status: 'healthy' | 'warning' | 'error';
    latency: string;
}

export interface Deployment {
    id: string;
    version: string;
    title: string;
    deployedBy: string;
    deployedAt: string;
    isCurrent: boolean;
}

export interface ProjectAgentLog {
    id: string;
    timestamp: string;
    agentName: string;
    agentColor: string;
    level: 'info' | 'debug' | 'warn' | 'error';
    message: string;
}
