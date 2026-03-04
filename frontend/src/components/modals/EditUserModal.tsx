// EditUserModal Component
import { useState, useEffect } from 'react';
import type { User } from '../../api/users';

interface EditUserModalProps {
    isOpen: boolean;
    onClose: () => void;
    user: User | null;
    onSave: (userId: string, data: { name: string; avatar?: string }) => Promise<void>;
}

export function EditUserModal({ isOpen, onClose, user, onSave }: EditUserModalProps) {
    const [name, setName] = useState('');
    const [avatarUrl, setAvatarUrl] = useState('');
    const [isSaving, setIsSaving] = useState(false);
    const [error, setError] = useState('');

    useEffect(() => {
        if (user) {
            setName(user.name);
            // Only set avatarUrl if it's a URL (not initials)
            setAvatarUrl(user.avatar.startsWith('http') ? user.avatar : '');
        }
    }, [user]);

    if (!isOpen || !user) return null;

    const handleSave = async () => {
        if (!name.trim()) {
            setError('Name is required');
            return;
        }

        setIsSaving(true);
        setError('');

        try {
            await onSave(user.id, {
                name: name.trim(),
                avatar: avatarUrl.trim() || undefined,
            });
            onClose();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to update user');
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose}></div>
            <div className="relative w-full max-w-lg bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">

                {/* Header */}
                <div className="px-6 py-5 border-b border-border flex justify-between items-center bg-muted">
                    <div>
                        <h2 className="text-lg font-bold text-card-foreground">Edit User</h2>
                        <p className="text-sm text-muted-foreground">Update user information</p>
                    </div>
                    <button onClick={onClose} className="text-muted-foreground hover:text-card-foreground transition-colors">
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {/* Body */}
                <div className="p-6 flex flex-col gap-4">
                    {/* Error Message */}
                    {error && (
                        <div className="px-4 py-3 bg-red-50 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 rounded-lg flex items-start gap-2">
                            <span className="material-symbols-outlined text-red-600 dark:text-red-400 text-[20px]">error</span>
                            <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
                        </div>
                    )}

                    {/* Name Field */}
                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">
                            Name <span className="text-red-500">*</span>
                        </label>
                        <input
                            type="text"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            placeholder="Enter user name"
                            className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
                        />
                    </div>

                    {/* Avatar URL Field */}
                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">
                            Avatar URL (optional)
                        </label>
                        <input
                            type="url"
                            value={avatarUrl}
                            onChange={(e) => setAvatarUrl(e.target.value)}
                            placeholder="https://example.com/avatar.jpg"
                            className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
                        />
                        <p className="mt-1 text-xs text-muted-foreground">
                            Leave empty to use auto-generated initials
                        </p>
                    </div>

                    {/* Read-only Email */}
                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">
                            Email
                        </label>
                        <input
                            type="email"
                            value={user.email}
                            disabled
                            className="w-full bg-muted/50 border border-border rounded-lg px-3 py-2.5 text-sm text-muted-foreground cursor-not-allowed"
                        />
                        <p className="mt-1 text-xs text-muted-foreground">
                            Email cannot be changed
                        </p>
                    </div>
                </div>

                {/* Footer */}
                <div className="px-6 py-4 border-t border-border bg-muted flex justify-end gap-3">
                    <button
                        onClick={onClose}
                        disabled={isSaving}
                        className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors disabled:opacity-50"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={handleSave}
                        disabled={!name.trim() || isSaving}
                        className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        {isSaving ? (
                            <>
                                <span className="animate-spin material-symbols-outlined text-[18px]">progress_activity</span>
                                Saving...
                            </>
                        ) : (
                            'Save Changes'
                        )}
                    </button>
                </div>
            </div>
        </div>
    );
}
