/**
 * ProjectMembersPanel - Manage project members (Owner only)
 *
 * Lists members, allows add by selecting user from system list, update role, remove.
 * Requires ManageMembers permission (Owner).
 */

import { useState, useEffect } from 'react';
import {
  getInviteableUsers,
  addProjectMember,
  updateProjectMember,
  removeProjectMember,
  type InviteableUser,
  type ProjectMember,
} from '../../api/projects';
import { logger } from '@/lib/logger';

type ProjectRole =
  | 'owner'
  | 'admin'
  | 'product_owner'
  | 'developer'
  | 'business_analyst'
  | 'quality_assurance'
  | 'viewer';

const PROJECT_ROLES: { value: ProjectRole; label: string }[] = [
  { value: 'owner', label: 'Owner' },
  { value: 'admin', label: 'Admin' },
  { value: 'product_owner', label: 'Product Owner' },
  { value: 'developer', label: 'Developer' },
  { value: 'business_analyst', label: 'Business Analyst' },
  { value: 'quality_assurance', label: 'QA' },
  { value: 'viewer', label: 'Viewer' },
];

interface ProjectMembersPanelProps {
  projectId: string;
  canManageMembers: boolean;
  members: ProjectMember[];
  setMembers: React.Dispatch<React.SetStateAction<ProjectMember[]>>;
  loading?: boolean;
  onRefresh?: () => void;
}

export function ProjectMembersPanel({
  projectId,
  canManageMembers,
  members,
  setMembers,
  loading = false,
  onRefresh,
}: ProjectMembersPanelProps) {
  const [inviteableUsers, setInviteableUsers] = useState<InviteableUser[]>([]);
  const [inviteableLoading, setInviteableLoading] = useState(false);
  const [selectedUserId, setSelectedUserId] = useState('');
  const [selectedRole, setSelectedRole] = useState<ProjectRole>('developer');
  const [adding, setAdding] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const [editingUserId, setEditingUserId] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);

  useEffect(() => {
    if (!canManageMembers || !projectId) return;
    setInviteableLoading(true);
    getInviteableUsers(projectId)
      .then(setInviteableUsers)
      .catch(() => setInviteableUsers([]))
      .finally(() => setInviteableLoading(false));
  }, [projectId, canManageMembers, members]);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedUserId || !canManageMembers) return;
    setAdding(true);
    setAddError(null);
    try {
      const added = await addProjectMember(projectId, {
        user_id: selectedUserId,
        roles: [selectedRole],
      });
      setMembers((prev) =>
        [...prev, added].sort((a, b) => a.name.localeCompare(b.name))
      );
      setSelectedUserId('');
      onRefresh?.();
    } catch (err) {
      setAddError(err instanceof Error ? err.message : 'Failed to add member');
    } finally {
      setAdding(false);
    }
  };

  const handleUpdateRole = async (userId: string, roles: ProjectRole[]) => {
    if (!canManageMembers) return;
    setUpdating(true);
    try {
      const updated = await updateProjectMember(projectId, userId, { roles });
      setMembers((prev) =>
        prev.map((m) => (m.id === userId ? { ...m, roles: updated.roles } : m))
      );
      setEditingUserId(null);
      onRefresh?.();
    } catch (err) {
      logger.error('Failed to update role:', err);
    } finally {
      setUpdating(false);
    }
  };

  const handleRemove = async (userId: string, name: string) => {
    if (!canManageMembers) return;
    if (!window.confirm(`Remove ${name} from this project?`)) return;
    try {
      await removeProjectMember(projectId, userId);
      setMembers((prev) => prev.filter((m) => m.id !== userId));
      onRefresh?.();
    } catch (err) {
      logger.error('Failed to remove member:', err);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {canManageMembers && (
        <form onSubmit={handleAdd} className="flex flex-wrap gap-2 items-end">
          <div className="flex-1 min-w-[220px]">
            <label className="block text-xs font-medium text-muted-foreground mb-1">
              Select user
            </label>
            <select
              value={selectedUserId}
              onChange={(e) => setSelectedUserId(e.target.value)}
              disabled={inviteableLoading}
              className="w-full px-3 py-2 text-sm border border-border rounded-lg bg-card text-card-foreground"
            >
              <option value="">
                {inviteableLoading
                  ? 'Loading...'
                  : inviteableUsers.length === 0
                    ? 'No more users to add'
                    : '-- Select user --'}
              </option>
              {inviteableUsers.map((u) => (
                <option key={u.id} value={u.id}>
                  {u.name} ({u.email})
                </option>
              ))}
            </select>
          </div>
          <div className="w-40">
            <label className="block text-xs font-medium text-muted-foreground mb-1">
              Role
            </label>
            <select
              value={selectedRole}
              onChange={(e) => setSelectedRole(e.target.value as ProjectRole)}
              className="w-full px-3 py-2 text-sm border border-border rounded-lg bg-card text-card-foreground"
            >
              {PROJECT_ROLES.map((r) => (
                <option key={r.value} value={r.value}>
                  {r.label}
                </option>
              ))}
            </select>
          </div>
          <button
            type="submit"
            disabled={!selectedUserId || adding || inviteableUsers.length === 0}
            className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-medium rounded-lg disabled:opacity-50"
          >
            {adding ? 'Adding...' : 'Add'}
          </button>
          {addError && (
            <p className="w-full text-xs text-red-500">{addError}</p>
          )}
        </form>
      )}

      <div className="border border-border rounded-lg overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-muted/50">
            <tr>
              <th className="text-left px-4 py-2 font-medium text-card-foreground">Name</th>
              <th className="text-left px-4 py-2 font-medium text-card-foreground">Email</th>
              <th className="text-left px-4 py-2 font-medium text-card-foreground">Role</th>
              {canManageMembers && (
                <th className="text-right px-4 py-2 font-medium text-card-foreground">
                  Actions
                </th>
              )}
            </tr>
          </thead>
          <tbody>
            {members.map((m) => (
              <tr key={m.id} className="border-t border-border">
                <td className="px-4 py-2 text-card-foreground">{m.name}</td>
                <td className="px-4 py-2 text-muted-foreground">{m.email}</td>
                <td className="px-4 py-2">
                  {editingUserId === m.id && canManageMembers ? (
                    <select
                      value={m.roles[0] ?? 'viewer'}
                      onChange={(e) =>
                        handleUpdateRole(m.id, [e.target.value as ProjectRole])
                      }
                      disabled={updating}
                      className="px-2 py-1 text-sm border border-border rounded bg-card"
                    >
                      {PROJECT_ROLES.map((r) => (
                        <option key={r.value} value={r.value}>
                          {r.label}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <span className="text-muted-foreground">
                      {m.roles.map((r) => PROJECT_ROLES.find((x) => x.value === r)?.label ?? r).join(', ')}
                    </span>
                  )}
                </td>
                {canManageMembers && (
                  <td className="px-4 py-2 text-right">
                    {editingUserId === m.id ? (
                      <button
                        onClick={() => setEditingUserId(null)}
                        className="text-xs text-muted-foreground hover:text-card-foreground"
                      >
                        Done
                      </button>
                    ) : (
                      <>
                        <button
                          onClick={() => setEditingUserId(m.id)}
                          className="text-xs text-primary hover:underline mr-2"
                        >
                          Edit
                        </button>
                        {!m.roles.includes('owner') && (
                          <button
                            onClick={() => handleRemove(m.id, m.name)}
                            className="text-xs text-red-500 hover:underline"
                          >
                            Remove
                          </button>
                        )}
                      </>
                    )}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
