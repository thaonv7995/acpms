/**
 * User Types
 * Shared types for user management across the application
 */

// Global system roles
export type SystemRole = 'admin' | 'product_owner' | 'business_analyst' | 'developer' | 'quality_assurance' | 'viewer';

// Project-specific roles
export type ProjectRole = 'owner' | 'admin' | 'product_owner' | 'developer' | 'business_analyst' | 'quality_assurance' | 'viewer';

// Legacy single role type (for backwards compatibility)
export type UserRole = 'admin' | 'developer' | 'viewer' | 'product_owner' | 'business_analyst' | 'tester';

export type UserStatus = 'active' | 'inactive' | 'pending';

export interface User {
  id: string;
  name: string;
  email: string;
  globalRoles: SystemRole[];
  status: UserStatus;
  avatar: string;
  agentPaired?: string;
  lastActive: string;
  createdAt: string;
}

export interface UserStats {
  total: number;
  active: number;
  agentsPaired: number;
  pending: number;
}
