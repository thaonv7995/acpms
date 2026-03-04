// Mock data for testing
export const mockProject = {
  id: 'proj-test-1',
  name: 'Test Project',
  description: 'Test Description',
  icon: 'folder',
  iconColor: 'blue' as const,
  techStack: ['React', 'TypeScript'],
  status: 'active_coding' as const,
  statusLabel: 'Active Coding',
  statusColor: 'blue' as const,
  progress: 65,
  agentIcon: 'smart_toy',
  lastActivity: '2 hours ago',
  agentCount: 3,
};

export const mockTask = {
  id: 'task-test-1',
  title: 'Test Task',
  description: 'Test task description',
  type: 'feature' as const,
  status: 'todo' as const,
  priority: 'medium' as const,
  progress: 0,
};

export const mockUser = {
  id: 'user-test-1',
  name: 'Test User',
  email: 'test@example.com',
  role: 'developer' as const,
  status: 'active' as const,
  avatar: 'TU',
  lastActive: '5m ago',
  createdAt: '2024-01-01',
};

export const mockMergeRequest = {
  id: 'mr-test-1',
  title: 'Test MR',
  description: 'Test MR description',
  status: 'open' as const,
  author: 'Test Author',
  createdAt: '2024-01-01',
  updatedAt: '2024-01-02',
};

export const mockSettings = {
  gitlab: {
    url: 'https://gitlab.example.com',
    token: 'test-token',
  },
  claude: {
    apiKey: 'test-api-key',
  },
};
