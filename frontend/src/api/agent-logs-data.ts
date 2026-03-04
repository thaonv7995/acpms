// Agent Logs Mock Data
export type AgentLogType = 'thought' | 'tool_use' | 'permission_request';

export interface AgentLogEntry {
    id: string;
    timestamp: string;
    agentName: string;
    agentColor: string;
    type: AgentLogType;
    message: string;
    highlight?: string;
    // Tool use specific fields
    command?: string;
    output?: string;
    // Permission request specific fields
    permissionType?: 'file_write' | 'api_call' | 'system_command';
    permissionDetails?: string;
    status?: 'pending' | 'approved' | 'denied';
}

export interface AgentStatus {
    id: string;
    name: string;
    status: 'active' | 'idle' | 'error';
    color: string;
    cpu: number;
    memory: number;
    uptime: string;
}

export const mockAgentStatuses: AgentStatus[] = [
    {
        id: 'agent-1',
        name: 'Agent-Alpha',
        status: 'active',
        color: 'text-blue-400',
        cpu: 45,
        memory: 512,
        uptime: '2h 34m'
    },
    {
        id: 'agent-2',
        name: 'Agent-Beta',
        status: 'active',
        color: 'text-purple-400',
        cpu: 32,
        memory: 384,
        uptime: '1h 12m'
    },
    {
        id: 'agent-3',
        name: 'Agent-Gamma',
        status: 'idle',
        color: 'text-green-400',
        cpu: 5,
        memory: 128,
        uptime: '45m'
    },
    {
        id: 'agent-4',
        name: 'QA-Bot',
        status: 'idle',
        color: 'text-teal-400',
        cpu: 2,
        memory: 96,
        uptime: '3h 21m'
    }
];

export const mockLogs: AgentLogEntry[] = [
    // Thought entries
    {
        id: 'log-1',
        timestamp: '10:42:05',
        agentName: 'Agent-Alpha',
        agentColor: 'text-blue-400',
        type: 'thought',
        message: 'Analyzing project structure to identify refactoring opportunities in the authentication module...'
    },
    {
        id: 'log-2',
        timestamp: '10:42:12',
        agentName: 'Agent-Beta',
        agentColor: 'text-purple-400',
        type: 'thought',
        message: 'Detected outdated dependencies. Preparing to update package.json with latest compatible versions.'
    },
    // Tool use entries
    {
        id: 'log-3',
        timestamp: '10:42:18',
        agentName: 'Agent-Alpha',
        agentColor: 'text-blue-400',
        type: 'tool_use',
        message: 'Executing command',
        command: 'git diff HEAD~1 src/auth/',
        output: '+45 lines, -23 lines modified'
    },
    {
        id: 'log-4',
        timestamp: '10:42:25',
        agentName: 'Agent-Beta',
        agentColor: 'text-purple-400',
        type: 'tool_use',
        message: 'Running tests',
        command: 'npm test -- auth.test.ts',
        output: '✓ 12 tests passed'
    },
    // Permission request entries
    {
        id: 'log-5',
        timestamp: '10:42:30',
        agentName: 'Agent-Alpha',
        agentColor: 'text-blue-400',
        type: 'permission_request',
        message: 'Requesting permission to modify authentication configuration',
        permissionType: 'file_write',
        permissionDetails: 'src/config/auth.config.ts - Add OAuth2 providers',
        status: 'pending'
    },
    {
        id: 'log-6',
        timestamp: '10:42:45',
        agentName: 'Agent-Gamma',
        agentColor: 'text-green-400',
        type: 'thought',
        message: 'Generating unit tests for newly created API endpoints to ensure 80% coverage target.'
    },
    {
        id: 'log-7',
        timestamp: '10:43:02',
        agentName: 'QA-Bot',
        agentColor: 'text-teal-400',
        type: 'tool_use',
        message: 'Running integration tests',
        command: 'npm run test:integration',
        output: '✓ All 24 integration tests passed'
    },
    {
        id: 'log-8',
        timestamp: '10:43:15',
        agentName: 'Agent-Beta',
        agentColor: 'text-purple-400',
        type: 'permission_request',
        message: 'Requesting permission to execute system command',
        permissionType: 'system_command',
        permissionDetails: 'npm audit fix --force',
        status: 'pending'
    },
    {
        id: 'log-9',
        timestamp: '10:43:28',
        agentName: 'Agent-Alpha',
        agentColor: 'text-blue-400',
        type: 'thought',
        message: 'Refactoring complete. Preparing to commit changes with descriptive commit message.'
    },
    {
        id: 'log-10',
        timestamp: '10:43:35',
        agentName: 'Agent-Gamma',
        agentColor: 'text-green-400',
        type: 'tool_use',
        message: 'Generating test coverage report',
        command: 'npm run coverage',
        output: 'Coverage: 82.5% statements, 78.3% branches'
    }
];
