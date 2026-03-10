/**
 * Users API - Real Backend Integration
 *
 * Migrated from mock API to real backend endpoints
 * Uses userMapper to transform backend User types to frontend User types
 */

import { apiGetFull, apiPost, apiPut, apiDelete, API_PREFIX, type ApiResponse } from './client';
import {
  mapBackendUser,
  calculateUserStats,
  applyUserFilters,
  type BackendUser,
  type UserFilters,
} from '../mappers/userMapper';
import type { User, UserStats, UserRole, UserStatus, SystemRole } from '../types/user';
import { logger } from '@/lib/logger';
import { filterHiddenServiceAccounts } from '@/lib/hiddenServiceAccounts';

// Re-export types for backward compatibility
export type { User, UserStats, UserRole, UserStatus };

export interface UsersQueryParams {
  page?: number;
  limit?: number;
  search?: string;
  role?: SystemRole;
  status?: UserStatus;
}

export interface UsersPageMetadata {
  page: number;
  page_size: number;
  total_count: number;
  total_pages: number;
  has_more: boolean;
  stats?: {
    total: number;
    active: number;
    agents_paired: number;
    pending: number;
  };
}

export async function getUsersPage(
  params?: UsersQueryParams
): Promise<ApiResponse<BackendUser[]>> {
  const query = params
    ? '?' +
      new URLSearchParams(
        Object.entries(params)
          .filter(([, value]) => value != null && value !== '')
          .map(([key, value]) => [key, String(value)])
      ).toString()
    : '';

  return apiGetFull<BackendUser[]>(`${API_PREFIX}/users${query}`);
}

/**
 * Get user statistics (computed from user list)
 */
export async function getUserStats(): Promise<UserStats> {
  try {
    const response = await getUsersPage({ page: 1, limit: 1 });
    const stats = (response.metadata as UsersPageMetadata | undefined)?.stats;

    if (stats) {
      return {
        total: stats.total,
        active: stats.active,
        agentsPaired: stats.agents_paired,
        pending: stats.pending,
      };
    }

    const frontendUsers = filterHiddenServiceAccounts(response.data ?? []).map(mapBackendUser);
    return calculateUserStats(frontendUsers);
  } catch (error) {
    logger.error('Failed to fetch user stats:', error);
    throw new Error('Failed to load user statistics. Please try again.');
  }
}

/**
 * Get users with optional filters
 * Filters are applied client-side for MVP
 */
export async function getUsers(filters?: UserFilters): Promise<User[]> {
  try {
    const response = await getUsersPage({
      page: 1,
      limit: 500,
      search: filters?.search,
      role: filters?.role,
      status: filters?.status,
    });
    const frontendUsers = filterHiddenServiceAccounts(response.data ?? []).map(mapBackendUser);
    return applyUserFilters(frontendUsers, filters);
  } catch (error) {
    logger.error('Failed to fetch users:', error);
    throw new Error('Failed to load users. Please try again.');
  }
}

/** Request body for admin creating a user (invite flow) */
export interface CreateUserRequest {
  email: string;
  name: string;
  password: string;
  global_roles?: string[];
}

/**
 * Create new user (admin only)
 * Backend: POST /api/v1/users
 */
export async function createUser(data: CreateUserRequest): Promise<User> {
  const response = await apiPost<BackendUser>(`${API_PREFIX}/users`, data);
  return mapBackendUser(response);
}

/**
 * Update user
 * Backend: PUT /api/v1/users/{id}
 * Accepts: name, avatar_url, gitlab_username (all optional)
 */
export async function updateUser(userId: string, userData: Partial<User>): Promise<User> {
  try {
    // Map frontend User fields to backend UpdateUserRequest fields
    const backendUpdate: {
      name?: string;
      avatar_url?: string;
      gitlab_username?: string;
    } = {};

    if (userData.name !== undefined) {
      backendUpdate.name = userData.name;
    }

    // Note: Frontend has 'avatar' field (string or initials)
    // Backend expects 'avatar_url' (URL or null)
    // Only update if it's a URL (starts with http)
    if (userData.avatar && userData.avatar.startsWith('http')) {
      backendUpdate.avatar_url = userData.avatar;
    }

    const response = await apiPut<BackendUser>(`${API_PREFIX}/users/${userId}`, backendUpdate);
    return mapBackendUser(response);
  } catch (error) {
    logger.error('Failed to update user:', error);
    throw new Error('Failed to update user. Please try again.');
  }
}

/**
 * Delete user
 * Backend: DELETE /api/v1/users/{id}
 */
export async function deleteUser(userId: string): Promise<void> {
  try {
    await apiDelete(`${API_PREFIX}/users/${userId}`);
  } catch (error) {
    logger.error('Failed to delete user:', error);
    throw new Error('Failed to delete user. Please try again.');
  }
}
