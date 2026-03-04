import { apiGet, apiPost, API_PREFIX } from './client';
import type { SprintDto } from './generated/models';

export type SprintCarryOverMode = 'move_to_next' | 'move_to_backlog' | 'keep_in_closed';

export interface CreateNextSprintPayload {
  name?: string;
  start_date?: string;
  end_date?: string;
  goal?: string;
}

export interface CloseSprintPayload {
  carry_over_mode: SprintCarryOverMode;
  next_sprint_id?: string;
  create_next_sprint?: CreateNextSprintPayload;
  reason?: string;
}

export interface CloseSprintResult {
  closedSprintId: string;
  movedTaskCount: number;
  movedToSprintId?: string | null;
  carryOverMode: SprintCarryOverMode;
}

export interface SprintOverview {
  sprintId: string;
  projectId: string;
  totalTasks: number;
  doneTasks: number;
  canceledTasks: number;
  remainingTasks: number;
  completionRate: number;
  movedInCount: number;
  movedOutCount: number;
}

export interface CreateProjectSprintPayload {
  name: string;
  description?: string;
  goal?: string;
  sequence?: number;
  start_date?: string;
  end_date?: string;
}

export type SprintWithRoadmapFields = SprintDto & {
  sequence?: number;
  goal?: string | null;
  closed_at?: string | null;
  closed_by?: string | null;
};

export async function createProjectSprint(
  projectId: string,
  payload: CreateProjectSprintPayload,
): Promise<SprintWithRoadmapFields> {
  return apiPost<SprintWithRoadmapFields>(`${API_PREFIX}/projects/${projectId}/sprints`, {
    project_id: projectId,
    ...payload,
  });
}

export async function closeProjectSprint(
  projectId: string,
  sprintId: string,
  payload: CloseSprintPayload,
): Promise<CloseSprintResult> {
  return apiPost<CloseSprintResult>(
    `${API_PREFIX}/projects/${projectId}/sprints/${sprintId}/close`,
    payload,
  );
}

export async function activateProjectSprint(
  projectId: string,
  sprintId: string,
): Promise<SprintWithRoadmapFields> {
  return apiPost<SprintWithRoadmapFields>(
    `${API_PREFIX}/projects/${projectId}/sprints/${sprintId}/activate`,
    {},
  );
}

export async function getSprintOverview(
  projectId: string,
  sprintId: string,
): Promise<SprintOverview> {
  return apiGet<SprintOverview>(`${API_PREFIX}/projects/${projectId}/sprints/${sprintId}/overview`);
}
