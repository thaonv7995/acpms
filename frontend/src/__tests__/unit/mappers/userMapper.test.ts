import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mapBackendUser, type BackendUser } from '../../../mappers/userMapper';

function buildBackendUser(overrides?: Partial<BackendUser>): BackendUser {
  return {
    id: '11111111-1111-1111-1111-111111111111',
    email: 'admin@example.com',
    name: 'Admin User',
    avatar_url: null,
    gitlab_id: null,
    gitlab_username: null,
    global_roles: ['admin'],
    created_at: '2026-03-01T10:00:00.000Z',
    updated_at: '2026-03-05T10:00:00.000Z',
    ...overrides,
  };
}

describe('userMapper', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-03-05T10:00:00.000Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('maps recently active users to active status', () => {
    const user = mapBackendUser(buildBackendUser());
    expect(user.status).toBe('active');
  });

  it('maps users with 7-29 days since update to inactive status', () => {
    const user = mapBackendUser(
      buildBackendUser({
        updated_at: '2026-02-20T10:00:00.000Z',
      })
    );
    expect(user.status).toBe('inactive');
  });

  it('maps users with 30+ days since update to pending status', () => {
    const user = mapBackendUser(
      buildBackendUser({
        updated_at: '2026-01-20T10:00:00.000Z',
      })
    );
    expect(user.status).toBe('pending');
  });

  it('falls back to viewer role when backend returns empty global_roles', () => {
    const user = mapBackendUser(
      buildBackendUser({
        global_roles: [],
      })
    );
    expect(user.globalRoles).toEqual(['viewer']);
  });
});
