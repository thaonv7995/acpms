import { apiGet, apiPost, API_PREFIX } from './client';

export type MRStatus = 'open' | 'pending_review' | 'merged' | 'closed';

export interface MergeRequest {
  id: string;
  taskId: string;
  projectId: string;
  projectName: string;
  latestAttemptId?: string | null;
  webUrl: string;
  mrNumber: number;
  title: string;
  description: string;
  status: MRStatus;
  author: {
    name: string;
    avatar?: string;
    isAgent: boolean;
  };
  branch: {
    source: string;
    target: string;
  };
  changes: {
    files: number;
    additions: number;
    deletions: number;
  };
  createdAt: string;
  updatedAt: string;
  labels: string[];
}

export interface MRStats {
  open: number;
  pendingReview: number;
  merged: number;
  aiGenerated: number;
}

interface MergeRequestOverviewDto {
  id: string;
  task_id: string;
  project_id: string;
  project_name: string;
  title: string;
  description?: string | null;
  mr_number: number;
  status: string;
  web_url: string;
  author_name: string;
  author_avatar?: string | null;
  author_is_agent: boolean;
  source_branch: string;
  target_branch: string;
  changed_files: number;
  additions: number;
  deletions: number;
  latest_attempt_id?: string | null;
  created_at: string;
  updated_at: string;
}

interface MergeRequestStatsDto {
  open: number;
  pending_review: number;
  merged: number;
  ai_generated: number;
}

function normalizeStatus(status: string): MRStatus {
  switch (status) {
    case 'open':
    case 'pending_review':
    case 'merged':
    case 'closed':
      return status;
    case 'opened':
      return 'open';
    default:
      return 'open';
  }
}

function formatRelativeTime(isoDate: string): string {
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) {
    return isoDate;
  }

  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return 'just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

function mapMergeRequest(dto: MergeRequestOverviewDto): MergeRequest {
  const status = normalizeStatus(dto.status);
  const labels = [dto.project_name];
  if (dto.author_is_agent) {
    labels.push('ai-generated');
  }

  return {
    id: dto.id,
    taskId: dto.task_id,
    projectId: dto.project_id,
    projectName: dto.project_name,
    latestAttemptId: dto.latest_attempt_id ?? null,
    webUrl: dto.web_url,
    mrNumber: dto.mr_number,
    title: dto.title,
    description: dto.description ?? '',
    status,
    author: {
      name: dto.author_name,
      avatar: dto.author_avatar ?? undefined,
      isAgent: dto.author_is_agent,
    },
    branch: {
      source: dto.source_branch,
      target: dto.target_branch,
    },
    changes: {
      files: dto.changed_files ?? 0,
      additions: dto.additions ?? 0,
      deletions: dto.deletions ?? 0,
    },
    createdAt: formatRelativeTime(dto.created_at),
    updatedAt: formatRelativeTime(dto.updated_at),
    labels,
  };
}

export async function getMergeRequestStats(): Promise<MRStats> {
  const stats = await apiGet<MergeRequestStatsDto>(`${API_PREFIX}/merge-requests/stats`);
  return {
    open: stats.open,
    pendingReview: stats.pending_review,
    merged: stats.merged,
    aiGenerated: stats.ai_generated,
  };
}

export async function getMergeRequests(filters?: {
  status?: MRStatus;
  search?: string;
}): Promise<MergeRequest[]> {
  const params = new URLSearchParams();
  if (filters?.status) {
    params.set('status', filters.status);
  }
  if (filters?.search) {
    params.set('search', filters.search);
  }

  const query = params.toString();
  const path = query
    ? `${API_PREFIX}/merge-requests?${query}`
    : `${API_PREFIX}/merge-requests`;

  const data = await apiGet<MergeRequestOverviewDto[]>(path);
  return data.map(mapMergeRequest);
}

/**
 * Sync MR status from GitLab for the given projects.
 * Calls POST /projects/:id/sync for each project, then caller should refetch list + stats.
 */
export async function syncGitLabMRs(projectIds: string[]): Promise<void> {
  if (projectIds.length === 0) return;
  await Promise.all(
    projectIds.map((id) =>
      apiPost<unknown>(`${API_PREFIX}/projects/${id}/sync`, {})
    )
  );
}

export async function reviewMergeRequest(_mrId: string): Promise<void> {
  // Navigation to review UI is handled by page-level router logic.
}
