import { useState, useEffect } from 'react';
import { apiPut } from '../../api/client';
import type { SystemRole, User } from '../../types/user';

interface EditUserRolesModalProps {
    isOpen: boolean;
    onClose: () => void;
    user: User | null;
    onSuccess: () => void;
}

const AVAILABLE_ROLES: { value: SystemRole; label: string; description: string }[] = [
    { value: 'admin', label: 'Admin', description: 'Full system access and user management' },
    { value: 'product_owner', label: 'Product Owner', description: 'Manage product backlog and priorities' },
    { value: 'business_analyst', label: 'Business Analyst', description: 'Requirements analysis and documentation' },
    { value: 'developer', label: 'Developer', description: 'Code development and technical implementation' },
    { value: 'quality_assurance', label: 'QA', description: 'Testing and quality assurance' },
    { value: 'viewer', label: 'Viewer', description: 'Read-only access to projects' },
];

export function EditUserRolesModal({ isOpen, onClose, user, onSuccess }: EditUserRolesModalProps) {
    const [selectedRoles, setSelectedRoles] = useState<SystemRole[]>([]);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (user) {
            setSelectedRoles([...user.globalRoles]);
        }
    }, [user]);

    const toggleRole = (role: SystemRole) => {
        setSelectedRoles(prev => {
            if (prev.includes(role)) {
                if (prev.length === 1) return prev;
                return prev.filter(r => r !== role);
            }
            return [...prev, role];
        });
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!user) return;

        setError(null);
        setIsSubmitting(true);

        try {
            await apiPut(`/api/v1/users/${user.id}`, {
                global_roles: selectedRoles,
            });
            onSuccess();
            onClose();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to update roles');
        } finally {
            setIsSubmitting(false);
        }
    };

    const handleClose = () => {
        setError(null);
        onClose();
    };

    if (!isOpen || !user) return null;

    const hasChanges = JSON.stringify([...selectedRoles].sort()) !== JSON.stringify([...user.globalRoles].sort());

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={handleClose}></div>
            <div className="relative w-full max-w-md bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
                {/* Header */}
                <div className="px-6 py-5 border-b border-border flex justify-between items-center bg-muted">
                    <div>
                        <h2 className="text-lg font-bold text-card-foreground">Edit User Roles</h2>
                        <p className="text-sm text-muted-foreground">Assign global roles to user</p>
                    </div>
                    <button onClick={handleClose} className="text-muted-foreground hover:text-card-foreground transition-colors">
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {/* Body */}
                <form onSubmit={handleSubmit} className="flex flex-col">
                    <div className="p-6 flex flex-col gap-4">
                        {error && (
                            <div className="px-4 py-3 bg-red-50 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 rounded-lg flex items-start gap-2">
                                <span className="material-symbols-outlined text-red-600 dark:text-red-400 text-[20px]">error</span>
                                <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
                            </div>
                        )}

                        {/* User Info */}
                        <div className="bg-muted border border-border rounded-lg p-4">
                            <div className="flex items-center gap-3">
                                <div className={`size-10 rounded-full flex items-center justify-center text-white font-bold text-sm ${
                                    user.avatar.startsWith('http')
                                        ? 'bg-slate-300'
                                        : 'bg-gradient-to-br from-blue-500 to-purple-500'
                                }`}>
                                    {user.avatar.startsWith('http') ? (
                                        <img src={user.avatar} alt={user.name} className="size-full rounded-full object-cover" />
                                    ) : (
                                        user.avatar
                                    )}
                                </div>
                                <div className="flex-1">
                                    <p className="font-bold text-card-foreground">{user.name}</p>
                                    <p className="text-xs text-muted-foreground">{user.email}</p>
                                </div>
                            </div>
                        </div>

                        {/* Role Selection */}
                        <div>
                            <label className="block text-sm font-bold text-card-foreground mb-1.5">
                                Global Roles
                            </label>
                            <div className="space-y-2">
                                {AVAILABLE_ROLES.map((role) => (
                                    <label
                                        key={role.value}
                                        className={`flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-all ${
                                            selectedRoles.includes(role.value)
                                                ? 'bg-primary/5 border-primary dark:bg-primary/10'
                                                : 'bg-muted border-border hover:border-primary/50'
                                        }`}
                                    >
                                        <input
                                            type="checkbox"
                                            checked={selectedRoles.includes(role.value)}
                                            onChange={() => toggleRole(role.value)}
                                            className="mt-0.5 size-4 rounded border-border text-primary focus:ring-primary"
                                        />
                                        <div className="flex-1">
                                            <p className="text-sm font-medium text-card-foreground">{role.label}</p>
                                            <p className="text-xs text-muted-foreground">{role.description}</p>
                                        </div>
                                    </label>
                                ))}
                            </div>
                            <p className="text-xs text-muted-foreground mt-2">At least one role must be selected.</p>
                        </div>
                    </div>

                    {/* Footer */}
                    <div className="px-6 py-4 border-t border-border bg-muted flex justify-end gap-3">
                        <button
                            type="button"
                            onClick={handleClose}
                            disabled={isSubmitting}
                            className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors disabled:opacity-50"
                        >
                            Cancel
                        </button>
                        <button
                            type="submit"
                            disabled={isSubmitting || !hasChanges}
                            className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            {isSubmitting ? (
                                <>
                                    <span className="animate-spin material-symbols-outlined text-[18px]">progress_activity</span>
                                    Saving...
                                </>
                            ) : (
                                <>
                                    <span className="material-symbols-outlined text-[18px]">save</span>
                                    Save Changes
                                </>
                            )}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    );
}
