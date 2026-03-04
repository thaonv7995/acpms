/**
 * User Mapper - Transform backend User to frontend User
 *
 * Handles type conversion and fills in frontend-specific fields
 */

import type { SystemRole, UserStatus, User as FrontendUser, UserStats } from '../types/user';

// Backend User type (from Rust models)
export interface BackendUser {
  id: string;  // UUID as string
  email: string;
  name: string;
  avatar_url: string | null;
  gitlab_id: number | null;
  gitlab_username: string | null;
  global_roles: string[];
  created_at: string;  // ISO 8601 datetime
  updated_at: string;  // ISO 8601 datetime
}

/**
 * Generate avatar initials from name
 */
function generateAvatarInitials(name: string): string {
  const parts = name.trim().split(/\s+/);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
  return name.substring(0, 2).toUpperCase();
}

/**
 * Calculate last active relative time from updated_at
 */
function formatLastActive(updatedAt: string): string {
  const updated = new Date(updatedAt);
  const now = new Date();
  const diffMs = now.getTime() - updated.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return updated.toLocaleDateString();
}

/**
 * Derive user status from updated_at timestamp
 * Active: created/updated within 7 days (includes newly created users)
 * Inactive: updated within 30 days
 * Pending: very old account, no activity in 30+ days
 */
function deriveUserStatus(updatedAt: string, _createdAt: string): UserStatus {
  const updated = new Date(updatedAt);
  const now = new Date();
  const daysSinceUpdate = (now.getTime() - updated.getTime()) / 86400000;

  // Active if created or updated within 7 days (new users from admin invite or self-register)
  if (daysSinceUpdate < 7) {
    return 'active';
  }

  // Inactive if updated within 30 days
  if (daysSinceUpdate < 30) {
    return 'inactive';
  }

  // Old accounts with no recent activity
  return 'pending';
}

/**
 * Map backend User to frontend User
 */
export function mapBackendUser(backendUser: BackendUser): FrontendUser {
  const avatar = backendUser.avatar_url || generateAvatarInitials(backendUser.name);
  const status = deriveUserStatus(backendUser.updated_at, backendUser.created_at);
  const globalRoles = (
    backendUser.global_roles && backendUser.global_roles.length > 0
      ? backendUser.global_roles
      : ['viewer']
  ) as SystemRole[];

  return {
    id: backendUser.id,
    name: backendUser.name,
    email: backendUser.email,
    globalRoles,
    status,
    avatar,
    agentPaired: undefined, // Future feature: agent pairing not implemented yet
    lastActive: formatLastActive(backendUser.updated_at),
    createdAt: new Date(backendUser.created_at).toISOString().split('T')[0], // YYYY-MM-DD
  };
}

/**
 * Calculate user statistics from user list
 */
export function calculateUserStats(users: FrontendUser[]): UserStats {
  return {
    total: users.length,
    active: users.filter(u => u.status === 'active').length,
    agentsPaired: users.filter(u => u.agentPaired).length,
    pending: users.filter(u => u.status === 'pending').length,
  };
}

/**
 * Apply client-side filters to user list
 */
export interface UserFilters {
  role?: SystemRole;
  status?: UserStatus;
  search?: string;
}

export function applyUserFilters(users: FrontendUser[], filters?: UserFilters): FrontendUser[] {
  if (!filters) return users;

  let filtered = [...users];

  if (filters.role) {
    // Filter by checking if user has the role in their globalRoles array
    filtered = filtered.filter(u => u.globalRoles.includes(filters.role!));
  }

  if (filters.status) {
    filtered = filtered.filter(u => u.status === filters.status);
  }

  if (filters.search) {
    const query = filters.search.toLowerCase();
    filtered = filtered.filter(u =>
      u.name.toLowerCase().includes(query) ||
      u.email.toLowerCase().includes(query)
    );
  }

  return filtered;
}
