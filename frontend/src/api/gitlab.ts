import { apiGet, apiPost, API_PREFIX } from './client';
import { GitLabConfiguration, LinkGitLabProjectRequest, MergeRequestDb as MergeRequest } from '../shared/types';

export type { MergeRequest };

export async function linkGitLabProject(
    projectId: string,
    data: LinkGitLabProjectRequest
): Promise<GitLabConfiguration> {
    return apiPost<GitLabConfiguration>(
        `${API_PREFIX}/projects/${projectId}/gitlab/link`,
        data
    );
}

export async function getGitLabStatus(projectId: string): Promise<GitLabConfiguration | null> {
    return apiGet<GitLabConfiguration | null>(
        `${API_PREFIX}/projects/${projectId}/gitlab/status`
    );
}

export async function getTaskMergeRequests(taskId: string): Promise<MergeRequest[]> {
    return apiGet<MergeRequest[]>(
        `${API_PREFIX}/tasks/${taskId}/gitlab/merge_requests`
    );
}
