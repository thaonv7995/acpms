/**
 * Users API - Real Backend Integration
 *
 * Migrated from mock API to real backend endpoints
 * Uses userMapper to transform backend User types to frontend User types
 */

import { apiGet, apiPost, apiPut, apiDelete } from './client';
import {
  mapBackendUser,
  calculateUserStats,
  applyUserFilters,
  type BackendUser,
  type UserFilters,
} from '../mappers/userMapper';
import type { User, UserStats, UserRole, UserStatus } from '../types/user';
import { logger } from '@/lib/logger';

// Re-export types for backward compatibility
export type { User, UserStats, UserRole, UserStatus };

/**
 * Get user statistics (computed from user list)
 */
export async function getUserStats(): Promise<UserStats> {
  try {
    const backendUsers = await apiGet<BackendUser[]>('/users');
    const frontendUsers = backendUsers.map(mapBackendUser);
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
    const backendUsers = await apiGet<BackendUser[]>('/users');
    const frontendUsers = backendUsers.map(mapBackendUser);
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
  const response = await apiPost<BackendUser>('/api/v1/users', data);
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

    const response = await apiPut<BackendUser>(`/users/${userId}`, backendUpdate);
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
    await apiDelete(`/users/${userId}`);
  } catch (error) {
    logger.error('Failed to delete user:', error);
    throw new Error('Failed to delete user. Please try again.');
  }
}
