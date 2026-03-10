import { useState, useEffect, useMemo } from 'react';
import { AppShell } from '../components/layout/AppShell';
import { USER_MANAGEMENT_PAGE_SIZE, useUsers } from '../hooks/useUsers';
import { useDebouncedValue } from '../hooks/useDebouncedValue';
import { EditUserModal } from '../components/modals/EditUserModal';
import { DeleteUserDialog } from '../components/modals/DeleteUserDialog';
import { InviteUserModal } from '../components/modals/InviteUserModal';
import { EditUserRolesModal } from '../components/modals/EditUserRolesModal';
import { UserProfileModal } from '../components/modals/UserProfileModal';
import { updateUser, deleteUser } from '../api/users';
import { getCurrentUser } from '../api/auth';
import type { SystemRole, UserStatus, User } from '../types/user';
import { logger } from '@/lib/logger';

export function UserManagementPage() {
    const {
        users,
        stats,
        loading,
        error,
        filterByRole,
        filterByStatus,
        search,
        refreshUsers,
        page,
        setPage,
        totalPages,
        totalCount,
    } = useUsers();
    const [searchInput, setSearchInput] = useState('');
    const debouncedSearch = useDebouncedValue(searchInput, 300);
    const pageStart = users.length === 0 ? 0 : (page - 1) * USER_MANAGEMENT_PAGE_SIZE + 1;
    const pageEnd = users.length === 0 ? 0 : pageStart + users.length - 1;

    // Current user context
    const currentUser = useMemo(() => getCurrentUser(), []);
    const isCurrentUserAdmin = useMemo(() =>
        currentUser?.global_roles?.includes('admin') ?? false
    , [currentUser]);

    // Modal state
    const [isEditModalOpen, setIsEditModalOpen] = useState(false);
    const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
    const [isInviteModalOpen, setIsInviteModalOpen] = useState(false);
    const [isEditRolesModalOpen, setIsEditRolesModalOpen] = useState(false);
    const [isProfileModalOpen, setIsProfileModalOpen] = useState(false);
    const [selectedUser, setSelectedUser] = useState<User | null>(null);
    const [isDeleting, setIsDeleting] = useState(false);
    const [actionMenuUserId, setActionMenuUserId] = useState<string | null>(null);

    // Apply debounced search
    useEffect(() => {
        search(debouncedSearch);
    }, [debouncedSearch, search]);

    // Handlers
    const handleEditUser = (user: User) => {
        setSelectedUser(user);
        setIsEditModalOpen(true);
        setActionMenuUserId(null);
    };

    const handleDeleteUser = (user: User) => {
        setSelectedUser(user);
        setIsDeleteDialogOpen(true);
        setActionMenuUserId(null);
    };

    const handleEditRoles = (user: User) => {
        setSelectedUser(user);
        setIsEditRolesModalOpen(true);
        setActionMenuUserId(null);
    };

    const handleViewProfile = (user: User) => {
        setSelectedUser(user);
        setIsProfileModalOpen(true);
        setActionMenuUserId(null);
    };

    const handleSaveUser = async (userId: string, data: { name: string; avatar?: string }) => {
        await updateUser(userId, data);
        await refreshUsers();
    };

    const handleConfirmDelete = async (userId: string) => {
        setIsDeleting(true);
        try {
            await deleteUser(userId);
            await refreshUsers();
            setIsDeleteDialogOpen(false);
            setSelectedUser(null);
        } catch (err) {
            logger.error('Failed to delete user:', err);
        } finally {
            setIsDeleting(false);
        }
    };

    const handleExportCSV = () => {
        if (users.length === 0) return;

        const headers = ['Name', 'Email', 'Roles', 'Status', 'Last Active', 'Created At'];
        const rows = users.map(user => [
            user.name,
            user.email,
            user.globalRoles.join(', '),
            user.status,
            user.lastActive,
            user.createdAt,
        ]);

        const csvContent = [
            headers.join(','),
            ...rows.map(row => row.map(cell => `"${cell}"`).join(','))
        ].join('\n');

        const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
        const url = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url;
        link.download = `users-export-${new Date().toISOString().split('T')[0]}.csv`;
        link.click();
        URL.revokeObjectURL(url);
    };

    if (loading) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center">
                    <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary"></div>
                </div>
            </AppShell>
        );
    }

    if (error) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center text-red-500 dark:text-red-400">{error}</div>
            </AppShell>
        );
    }

    return (
        <AppShell>
            <div className="flex-1 overflow-y-auto p-6 md:p-8 scrollbar-hide bg-background">
                <div className="max-w-[1600px] mx-auto flex flex-col gap-6">

                    {/* Header */}
                    <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
                        <div>
                            <h1 className="text-3xl font-bold text-card-foreground mb-2">User & Role Management</h1>
                            <p className="text-muted-foreground text-sm max-w-2xl">
                                Manage system access, assign roles (PO, BA, Dev, Tester), and pair users with AI agents to automate workflows.
                            </p>
                        </div>
                        <div className="flex gap-2">
                            <button
                                onClick={handleExportCSV}
                                disabled={users.length === 0}
                                className="flex items-center gap-1.5 px-3 py-1.5 bg-card border border-border hover:bg-muted text-card-foreground text-xs font-medium rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                <span className="material-symbols-outlined text-[16px]">download</span>
                                Export CSV
                            </button>
                            <button
                                onClick={() => setIsInviteModalOpen(true)}
                                className="flex items-center gap-1.5 px-3 py-1.5 bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-bold rounded-lg shadow-sm shadow-primary/20 transition-all"
                            >
                                <span className="material-symbols-outlined text-[16px]">person_add</span>
                                Invite User
                            </button>
                        </div>
                    </div>

                    {/* Filters */}
                    <div className="flex flex-col md:flex-row gap-4 bg-card p-4 rounded-xl border border-border shadow-sm">
                        <div className="flex-1 relative">
                            <span className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground material-symbols-outlined text-[20px]">search</span>
                            <input
                                type="text"
                                value={searchInput}
                                onChange={(e) => setSearchInput(e.target.value)}
                                placeholder="Search users by name, email, or agent ID..."
                                className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg pl-10 pr-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary placeholder-muted-foreground"
                            />
                        </div>
                        <div className="flex gap-4">
                            <div className="relative min-w-[160px]">
                                <select
                                    onChange={(e) => filterByRole(e.target.value === 'all' ? null : e.target.value as SystemRole)}
                                    className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg pl-4 pr-10 py-2.5 appearance-none focus:ring-1 focus:ring-primary focus:border-primary"
                                >
                                    <option value="all">All Roles</option>
                                    <option value="admin">Admin</option>
                                    <option value="product_owner">Product Owner</option>
                                    <option value="business_analyst">Business Analyst</option>
                                    <option value="developer">Developer</option>
                                    <option value="quality_assurance">QA</option>
                                    <option value="viewer">Viewer</option>
                                </select>
                                <span className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground material-symbols-outlined text-[20px] pointer-events-none">expand_more</span>
                            </div>
                            <div className="relative min-w-[160px]">
                                <select
                                    onChange={(e) => filterByStatus(e.target.value === 'all' ? null : e.target.value as UserStatus)}
                                    className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg pl-4 pr-10 py-2.5 appearance-none focus:ring-1 focus:ring-primary focus:border-primary"
                                >
                                    <option value="all">Status: All</option>
                                    <option value="active">Active</option>
                                    <option value="inactive">Inactive</option>
                                    <option value="pending">Pending</option>
                                </select>
                                <span className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground material-symbols-outlined text-[20px] pointer-events-none">expand_more</span>
                            </div>
                        </div>
                    </div>

                    {/* Table Container */}
                    <div className="bg-card border border-border rounded-xl overflow-hidden shadow-sm">
                        <div className="overflow-x-auto">
                            <table className="w-full text-left border-collapse">
                                <thead>
                                    <tr className="bg-muted/50 border-b border-border text-muted-foreground text-xs uppercase tracking-wider font-bold">
                                        <th className="p-4 w-12">
                                            <input type="checkbox" className="rounded border-border bg-card text-primary" />
                                        </th>
                                        <th className="p-4">User</th>
                                        <th className="p-4">Role</th>
                                        <th className="p-4">Assigned Agent</th>
                                        <th className="p-4">Status</th>
                                        <th className="p-4">Last Active</th>
                                        <th className="p-4 text-right">Actions</th>
                                    </tr>
                                </thead>
                                <tbody className="text-sm text-card-foreground">
                                    {users.length === 0 ? (
                                        <tr>
                                            <td colSpan={7} className="p-12 text-center text-muted-foreground">
                                                No users found
                                            </td>
                                        </tr>
                                    ) : (
                                        users.map((user) => (
                                            <tr key={user.id} className="border-b border-border/50 hover:bg-muted/30 transition-colors">
                                                <td className="p-4">
                                                    <input type="checkbox" className="rounded border-border bg-card text-primary" />
                                                </td>
                                                <td className="p-4">
                                                    <div className="flex items-center gap-3">
                                                        <div className="size-10 rounded-full bg-gradient-to-br from-blue-500 to-purple-500 flex items-center justify-center text-white font-bold text-sm">
                                                            {user.avatar.startsWith('http') ? (
                                                                <img src={user.avatar} alt={user.name} className="size-full rounded-full object-cover" />
                                                            ) : (
                                                                user.avatar
                                                            )}
                                                        </div>
                                                        <div>
                                                            <p className="font-bold text-card-foreground">{user.name}</p>
                                                            <p className="text-muted-foreground text-xs">{user.email}</p>
                                                        </div>
                                                    </div>
                                                </td>
                                                <td className="p-4">
                                                    <div className="flex flex-wrap gap-1">
                                                        {user.globalRoles.map((role) => {
                                                            const roleColors: Record<string, { bg: string; text: string; dot: string }> = {
                                                                admin: { bg: 'bg-purple-100 dark:bg-purple-500/20 border-purple-200 dark:border-purple-500/30', text: 'text-purple-700 dark:text-purple-400', dot: 'bg-purple-500' },
                                                                product_owner: { bg: 'bg-orange-100 dark:bg-orange-500/20 border-orange-200 dark:border-orange-500/30', text: 'text-orange-700 dark:text-orange-400', dot: 'bg-orange-500' },
                                                                business_analyst: { bg: 'bg-cyan-100 dark:bg-cyan-500/20 border-cyan-200 dark:border-cyan-500/30', text: 'text-cyan-700 dark:text-cyan-400', dot: 'bg-cyan-500' },
                                                                developer: { bg: 'bg-green-100 dark:bg-green-500/20 border-green-200 dark:border-green-500/30', text: 'text-green-700 dark:text-green-400', dot: 'bg-green-500' },
                                                                quality_assurance: { bg: 'bg-pink-100 dark:bg-pink-500/20 border-pink-200 dark:border-pink-500/30', text: 'text-pink-700 dark:text-pink-400', dot: 'bg-pink-500' },
                                                                viewer: { bg: 'bg-slate-100 dark:bg-slate-500/20 border-slate-200 dark:border-slate-500/30', text: 'text-slate-700 dark:text-slate-400', dot: 'bg-slate-500' },
                                                            };
                                                            const colors = roleColors[role] || roleColors.viewer;
                                                            const displayName: Record<string, string> = {
                                                                admin: 'Admin', product_owner: 'PO', business_analyst: 'BA',
                                                                developer: 'Dev', quality_assurance: 'QA', viewer: 'Viewer'
                                                            };
                                                            return (
                                                                <span
                                                                    key={role}
                                                                    className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium border ${colors.bg} ${colors.text}`}
                                                                >
                                                                    <span className={`size-1.5 rounded-full ${colors.dot}`}></span>
                                                                    {displayName[role] || role}
                                                                </span>
                                                            );
                                                        })}
                                                    </div>
                                                </td>
                                                <td className="p-4">
                                                    {user.agentPaired ? (
                                                        <div className="flex items-center gap-2">
                                                            <span className="material-symbols-outlined text-purple-600 dark:text-purple-400 text-[18px]">smart_toy</span>
                                                            <span className="font-mono text-xs text-card-foreground">{user.agentPaired}</span>
                                                        </div>
                                                    ) : (
                                                        <span className="text-muted-foreground text-xs italic">Unassigned</span>
                                                    )}
                                                </td>
                                                <td className="p-4">
                                                    <span className={`inline-block px-2 py-0.5 rounded text-[11px] font-bold ${
                                                        user.status === 'active'
                                                            ? 'bg-green-100 dark:bg-green-500/20 text-green-700 dark:text-green-400 border border-green-200 dark:border-green-500/30'
                                                            : user.status === 'inactive'
                                                            ? 'bg-amber-100 dark:bg-amber-500/20 text-amber-700 dark:text-amber-400 border border-amber-200 dark:border-amber-500/30'
                                                            : 'bg-slate-100 dark:bg-slate-500/20 text-slate-700 dark:text-slate-400 border border-slate-200 dark:border-slate-500/30'
                                                    }`}>
                                                        {user.status}
                                                    </span>
                                                </td>
                                                <td className="p-4 text-muted-foreground text-xs">{user.lastActive}</td>
                                                <td className="p-4 text-right relative">
                                                    <button
                                                        onClick={() => setActionMenuUserId(actionMenuUserId === user.id ? null : user.id)}
                                                        className="text-muted-foreground hover:text-card-foreground transition-colors"
                                                    >
                                                        <span className="material-symbols-outlined text-[20px]">more_vert</span>
                                                    </button>
                                                    {actionMenuUserId === user.id && (
                                                        <div className="absolute right-12 top-8 bg-card border border-border rounded-lg shadow-xl py-1 z-10 min-w-[160px]">
                                                            {/* Show "My Profile" for own user */}
                                                            {currentUser?.id === user.id ? (
                                                                <button
                                                                    onClick={() => handleViewProfile(user)}
                                                                    className="w-full px-4 py-2 text-left text-sm text-card-foreground hover:bg-muted flex items-center gap-2"
                                                                >
                                                                    <span className="material-symbols-outlined text-[18px]">person</span>
                                                                    My Profile
                                                                </button>
                                                            ) : (
                                                                <button
                                                                    onClick={() => handleEditUser(user)}
                                                                    className="w-full px-4 py-2 text-left text-sm text-card-foreground hover:bg-muted flex items-center gap-2"
                                                                >
                                                                    <span className="material-symbols-outlined text-[18px]">edit</span>
                                                                    Edit User
                                                                </button>
                                                            )}
                                                            {/* Only admins can edit roles */}
                                                            {isCurrentUserAdmin && (
                                                                <button
                                                                    onClick={() => handleEditRoles(user)}
                                                                    className="w-full px-4 py-2 text-left text-sm text-card-foreground hover:bg-muted flex items-center gap-2"
                                                                >
                                                                    <span className="material-symbols-outlined text-[18px]">manage_accounts</span>
                                                                    Edit Roles
                                                                </button>
                                                            )}
                                                            {/* Only admins can delete, and can't delete self */}
                                                            {isCurrentUserAdmin && currentUser?.id !== user.id && (
                                                                <button
                                                                    onClick={() => handleDeleteUser(user)}
                                                                    className="w-full px-4 py-2 text-left text-sm text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-500/20 flex items-center gap-2"
                                                                >
                                                                    <span className="material-symbols-outlined text-[18px]">delete</span>
                                                                    Delete User
                                                                </button>
                                                            )}
                                                        </div>
                                                    )}
                                                </td>
                                            </tr>
                                        ))
                                    )}
                                </tbody>
                            </table>
                        </div>

                        {/* Pagination */}
                        <div className="px-6 py-4 border-t border-border bg-muted/50 flex items-center justify-between">
                            <span className="text-sm text-muted-foreground">
                                Showing <span className="text-card-foreground font-bold">{pageStart}</span> to <span className="text-card-foreground font-bold">{pageEnd}</span> of <span className="text-card-foreground font-bold">{totalCount}</span> users
                            </span>
                            <div className="flex items-center gap-2">
                                <span className="text-xs text-muted-foreground">Page {page} / {totalPages}</span>
                                <button
                                    type="button"
                                    disabled={page <= 1}
                                    onClick={() => setPage(page - 1)}
                                    className="px-4 py-2 bg-card border border-border rounded-lg text-sm text-muted-foreground hover:text-card-foreground hover:border-border/80 transition-colors disabled:opacity-50"
                                >
                                    Previous
                                </button>
                                <button
                                    type="button"
                                    disabled={page >= totalPages}
                                    onClick={() => setPage(page + 1)}
                                    className="px-4 py-2 bg-card border border-border rounded-lg text-sm text-muted-foreground hover:text-card-foreground hover:border-border/80 transition-colors disabled:opacity-50"
                                >
                                    Next
                                </button>
                            </div>
                        </div>
                    </div>

                    {/* Stats Grid */}
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
                        <div className="bg-card border border-border p-4 rounded-xl flex items-center gap-3 shadow-sm">
                            <div className="size-10 rounded-lg bg-blue-100 dark:bg-blue-500/20 flex items-center justify-center text-blue-600 dark:text-blue-400">
                                <span className="material-symbols-outlined text-2xl">group</span>
                            </div>
                            <div>
                                <p className="text-muted-foreground text-[10px] font-bold uppercase tracking-wide">Total Users</p>
                                <p className="text-2xl font-bold text-card-foreground">{stats?.total || 0}</p>
                            </div>
                        </div>

                        <div className="bg-card border border-border p-4 rounded-xl flex items-center gap-3 shadow-sm">
                            <div className="size-10 rounded-lg bg-emerald-100 dark:bg-emerald-500/20 flex items-center justify-center text-emerald-600 dark:text-emerald-400">
                                <span className="material-symbols-outlined text-2xl">how_to_reg</span>
                            </div>
                            <div>
                                <p className="text-muted-foreground text-[10px] font-bold uppercase tracking-wide">Active Users</p>
                                <p className="text-2xl font-bold text-card-foreground">{stats?.active || 0}</p>
                            </div>
                        </div>

                        <div className="bg-card border border-border p-4 rounded-xl flex items-center gap-3 shadow-sm">
                            <div className="size-10 rounded-lg bg-purple-100 dark:bg-purple-500/20 flex items-center justify-center text-purple-600 dark:text-purple-400">
                                <span className="material-symbols-outlined text-2xl">smart_toy</span>
                            </div>
                            <div>
                                <p className="text-muted-foreground text-[10px] font-bold uppercase tracking-wide">Agents Paired</p>
                                <p className="text-2xl font-bold text-card-foreground">{stats?.agentsPaired || 0}</p>
                            </div>
                        </div>

                        <div className="bg-card border border-border p-4 rounded-xl flex items-center gap-3 shadow-sm">
                            <div className="size-10 rounded-lg bg-orange-100 dark:bg-orange-500/20 flex items-center justify-center text-orange-600 dark:text-orange-400">
                                <span className="material-symbols-outlined text-2xl">pending_actions</span>
                            </div>
                            <div>
                                <p className="text-muted-foreground text-[10px] font-bold uppercase tracking-wide">Pending Invites</p>
                                <p className="text-2xl font-bold text-card-foreground">{stats?.pending || 0}</p>
                            </div>
                        </div>
                    </div>

                </div>
            </div>

            {/* Modals */}
            <EditUserModal
                isOpen={isEditModalOpen}
                onClose={() => setIsEditModalOpen(false)}
                user={selectedUser}
                onSave={handleSaveUser}
            />
            <DeleteUserDialog
                isOpen={isDeleteDialogOpen}
                onClose={() => setIsDeleteDialogOpen(false)}
                user={selectedUser}
                onConfirm={handleConfirmDelete}
                isDeleting={isDeleting}
            />
            <InviteUserModal
                isOpen={isInviteModalOpen}
                onClose={() => setIsInviteModalOpen(false)}
                onSuccess={refreshUsers}
            />
            <EditUserRolesModal
                isOpen={isEditRolesModalOpen}
                onClose={() => setIsEditRolesModalOpen(false)}
                user={selectedUser}
                onSuccess={refreshUsers}
            />
            <UserProfileModal
                isOpen={isProfileModalOpen}
                onClose={() => setIsProfileModalOpen(false)}
                user={selectedUser}
                onSuccess={refreshUsers}
            />
        </AppShell>
    );
}
