// Projects Mock API for demo data
import type {
    ProjectListItem,
    ProjectDetail,
    KanbanColumn,
    Requirement,
    InfrastructureService,
    Deployment,
    ProjectAgentLog
} from '../types/project';

// Simulated API delay
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

// Mock Project List Data
const mockProjectList: ProjectListItem[] = [
    {
        id: 'demo-1',
        name: 'E-Commerce Refactor v2',
        description: 'Full-stack e-commerce platform modernization',
        icon: 'shopping_cart',
        iconColor: 'orange',
        techStack: ['React', 'Node.js'],
        status: 'reviewing',
        statusLabel: 'Reviewing',
        statusColor: 'yellow',
        progress: 65,
        agentIcon: 'smart_toy',
        lastActivity: '2m ago',
        agentCount: 3,
    },
    {
        id: 'demo-2',
        name: 'Auth Service Migration',
        description: 'OAuth2 implementation with SSO support',
        icon: 'security',
        iconColor: 'blue',
        techStack: ['Python', 'OAuth'],
        status: 'active',
        statusLabel: 'Active',
        statusColor: 'blue',
        progress: 30,
        agentIcon: 'shield',
        lastActivity: '45m ago',
        agentCount: 2,
    },
    {
        id: 'demo-3',
        name: 'Analytics Dashboard',
        description: 'Real-time analytics and reporting dashboard',
        icon: 'analytics',
        iconColor: 'emerald',
        techStack: ['Vue.js', 'AWS'],
        status: 'active',
        statusLabel: 'Active',
        statusColor: 'blue',
        progress: 90,
        agentIcon: 'rocket_launch',
        lastActivity: '1h ago',
        agentCount: 1,
    },
];

// Mock Project Detail Data
const mockProjectDetails: Record<string, ProjectDetail> = {
    'demo-1': {
        id: 'demo-1',
        name: 'E-Commerce Refactor v2',
        repositoryUrl: 'gitlab.com/acpms/e-commerce',
        branch: 'main',
        status: 'active',
        lastDeploy: '2h ago',
        stats: { activeAgents: 4, pendingReview: 12, criticalBugs: 3, buildStatus: 98 },
    },
    'demo-2': {
        id: 'demo-2',
        name: 'Auth Service Migration',
        repositoryUrl: 'gitlab.com/acpms/auth-service',
        branch: 'develop',
        status: 'active',
        lastDeploy: '45m ago',
        stats: { activeAgents: 2, pendingReview: 5, criticalBugs: 1, buildStatus: 100 },
    },
    'demo-3': {
        id: 'demo-3',
        name: 'Analytics Dashboard',
        repositoryUrl: 'gitlab.com/acpms/analytics',
        branch: 'main',
        status: 'active',
        lastDeploy: '1h ago',
        stats: { activeAgents: 1, pendingReview: 3, criticalBugs: 0, buildStatus: 95 },
    },
};

// Mock Kanban Data
const mockKanbanColumns: KanbanColumn[] = [
    {
        id: 'todo',
        title: 'To Do',
        status: 'todo',
        color: 'slate',
        tasks: [
            { id: 't1', title: 'Implement OAuth2 Authentication Flow', type: 'feature', status: 'todo', priority: 'high', assignee: { id: 'a1', initial: 'A', color: 'bg-indigo-500' }, attachments: 2, createdAt: '2024-01-15T10:00:00Z' },
            { id: 't2', title: 'Fix memory leak in image processor', type: 'bug', status: 'todo', priority: 'critical', assignee: { id: 'a2', initial: 'B', color: 'bg-teal-500' }, attachments: 1, createdAt: '2024-01-15T11:00:00Z' },
        ],
    },
    {
        id: 'in_progress',
        title: 'In Progress',
        status: 'in_progress',
        color: 'primary',
        tasks: [
            { id: 't3', title: 'Refactor Database Schema for Users', description: 'Optimizing indices and splitting the user metadata table for better query performance.', type: 'refactor', status: 'in_progress', priority: 'high', progress: 65, agentWorking: { name: 'Agent-03', progress: 65 }, createdAt: '2024-01-14T09:00:00Z' },
        ],
    },
    {
        id: 'done',
        title: 'Done',
        status: 'done',
        color: 'green',
        tasks: [
            { id: 't4', title: 'Setup CI/CD pipeline', type: 'chore', status: 'done', priority: 'medium', assignee: { id: 'a3', initial: 'C', color: 'bg-green-500' }, createdAt: '2024-01-13T08:00:00Z' },
        ],
    },
];

// Mock Requirements
const mockRequirements: Requirement[] = [
    {
        id: 'REQ-101',
        project_id: 'proj-1',
        type: 'functional',
        title: 'User Authentication via OAuth2',
        content: 'System must support login via Google and GitHub providers with JWT session management.',
        description: 'System must support login via Google and GitHub providers with JWT session management.',
        status: 'approved',
        priority: 'high',
        created_by: 'user-1',
        created_at: '2024-01-15T10:00:00Z',
        updated_at: '2024-01-15T10:00:00Z'
    },
    {
        id: 'REQ-102',
        project_id: 'proj-1',
        type: 'technical',
        title: 'Database Schema Migration Strategy',
        content: 'Zero-downtime migration plan for the users table splitting metadata into separate relational entities.',
        description: 'Zero-downtime migration plan for the users table splitting metadata into separate relational entities.',
        status: 'in_review',
        priority: 'critical',
        created_by: 'user-1',
        created_at: '2024-01-16T10:00:00Z',
        updated_at: '2024-01-16T10:00:00Z'
    },
    {
        id: 'REQ-103',
        project_id: 'proj-1',
        type: 'non_functional',
        title: 'API Rate Limiting',
        content: 'Implement sliding window rate limiting: 1000 req/min for authenticated users, 60 req/min for public.',
        description: 'Implement sliding window rate limiting: 1000 req/min for authenticated users, 60 req/min for public.',
        status: 'draft',
        priority: 'medium',
        created_by: 'user-1',
        created_at: '2024-01-17T10:00:00Z',
        updated_at: '2024-01-17T10:00:00Z'
    },
];

// Mock Infrastructure
const mockInfrastructure: InfrastructureService[] = [
    { id: 's1', name: 'API Gateway', status: 'healthy', latency: '45ms' },
    { id: 's2', name: 'Auth Service', status: 'healthy', latency: '120ms' },
    { id: 's3', name: 'User Service', status: 'warning', latency: '850ms' },
    { id: 's4', name: 'Primary DB', status: 'healthy', latency: '12ms' },
];

// Mock Deployments
const mockDeployments: Deployment[] = [
    { id: 'd1', version: 'v2.1.0', title: 'Auth Hotfix', deployedBy: 'Agent-Alpha', deployedAt: '2h ago', isCurrent: true },
    { id: 'd2', version: 'v2.0.0', title: 'Major Release', deployedBy: 'Sarah Admin', deployedAt: 'Yesterday', isCurrent: false },
];

// Mock Agent Logs
const mockAgentLogs: ProjectAgentLog[] = [
    { id: 'l1', timestamp: '10:42:15', agentName: 'Agent-03', agentColor: 'text-primary', level: 'info', message: 'Started migration script generation for `users` table...' },
    { id: 'l2', timestamp: '10:42:12', agentName: 'Agent-03', agentColor: 'text-primary', level: 'debug', message: 'Fetching schema definition from information_schema.columns' },
    { id: 'l3', timestamp: '10:41:55', agentName: 'Agent-Beta', agentColor: 'text-teal-500', level: 'warn', message: 'Analyzed heap dump. Found potential leak in `ImageBuffer.process()`.' },
    { id: 'l4', timestamp: '10:41:48', agentName: 'System', agentColor: 'text-slate-500', level: 'info', message: 'Build #882 started triggered by commit 8f2a1c' },
    { id: 'l5', timestamp: '10:40:22', agentName: 'DevBot-Alpha', agentColor: 'text-purple-500', level: 'error', message: 'Failed to resolve dependency: react-scripts@5.0.1 (ETIMEDOUT)' },
];

// API Functions
export async function getMockProjectList(): Promise<ProjectListItem[]> {
    await delay(300);
    return mockProjectList;
}

export async function getMockProjectDetail(id: string): Promise<ProjectDetail | null> {
    await delay(250);
    return mockProjectDetails[id] || null;
}

export async function getMockKanbanColumns(_projectId: string): Promise<KanbanColumn[]> {
    await delay(200);
    return mockKanbanColumns;
}

export async function getMockRequirements(_projectId: string): Promise<Requirement[]> {
    await delay(200);
    return mockRequirements;
}

export async function getMockInfrastructure(_projectId: string): Promise<InfrastructureService[]> {
    await delay(150);
    return mockInfrastructure;
}

export async function getMockDeployments(_projectId: string): Promise<Deployment[]> {
    await delay(150);
    return mockDeployments;
}

export async function getMockAgentLogs(_projectId: string): Promise<ProjectAgentLog[]> {
    await delay(200);
    return mockAgentLogs;
}
