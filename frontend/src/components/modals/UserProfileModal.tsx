import { useState, useEffect } from 'react';
import { apiPut } from '../../api/client';
import type { User } from '../../types/user';

interface UserProfileModalProps {
    isOpen: boolean;
    onClose: () => void;
    user: User | null;
    onSuccess: () => void;
}

export function UserProfileModal({ isOpen, onClose, user, onSuccess }: UserProfileModalProps) {
    const [name, setName] = useState('');
    const [avatarUrl, setAvatarUrl] = useState('');
    const [currentPassword, setCurrentPassword] = useState('');
    const [newPassword, setNewPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [successMessage, setSuccessMessage] = useState<string | null>(null);
    const [activeTab, setActiveTab] = useState<'profile' | 'password'>('profile');

    useEffect(() => {
        if (user) {
            setName(user.name);
            setAvatarUrl(user.avatar.startsWith('http') ? user.avatar : '');
        }
    }, [user]);

    const handleProfileSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!user) return;

        setError(null);
        setSuccessMessage(null);
        setIsSubmitting(true);

        try {
            await apiPut(`/api/v1/users/${user.id}`, {
                name: name.trim(),
                avatar_url: avatarUrl.trim() || null,
            });
            setSuccessMessage('Profile updated successfully');
            onSuccess();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to update profile');
        } finally {
            setIsSubmitting(false);
        }
    };

    const handlePasswordSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!user) return;

        setError(null);
        setSuccessMessage(null);

        // Validation
        if (!currentPassword) {
            setError('Current password is required');
            return;
        }
        if (newPassword.length < 8) {
            setError('New password must be at least 8 characters');
            return;
        }
        if (newPassword !== confirmPassword) {
            setError('New passwords do not match');
            return;
        }

        setIsSubmitting(true);

        try {
            await apiPut(`/api/v1/users/${user.id}/password`, {
                current_password: currentPassword,
                new_password: newPassword,
            });
            setSuccessMessage('Password changed successfully');
            setCurrentPassword('');
            setNewPassword('');
            setConfirmPassword('');
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to change password');
        } finally {
            setIsSubmitting(false);
        }
    };

    const handleClose = () => {
        setError(null);
        setSuccessMessage(null);
        setCurrentPassword('');
        setNewPassword('');
        setConfirmPassword('');
        setActiveTab('profile');
        onClose();
    };

    if (!isOpen || !user) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={handleClose}></div>
            <div className="relative w-full max-w-lg bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
                {/* Header */}
                <div className="flex items-center justify-between p-6 border-b border-border">
                    <div className="flex items-center gap-3">
                        <div className="size-10 rounded-lg bg-primary/10 flex items-center justify-center">
                            <span className="material-symbols-outlined text-primary">person</span>
                        </div>
                        <h2 className="text-lg font-bold text-card-foreground">My Profile</h2>
                    </div>
                    <button
                        onClick={handleClose}
                        className="text-muted-foreground hover:text-card-foreground transition-colors"
                    >
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {/* Tabs */}
                <div className="flex border-b border-border">
                    <button
                        onClick={() => setActiveTab('profile')}
                        className={`flex-1 px-4 py-3 text-sm font-medium transition-colors ${
                            activeTab === 'profile'
                                ? 'text-primary border-b-2 border-primary bg-primary/5'
                                : 'text-muted-foreground hover:text-card-foreground'
                        }`}
                    >
                        <span className="material-symbols-outlined text-[18px] align-middle mr-2">badge</span>
                        Profile Info
                    </button>
                    <button
                        onClick={() => setActiveTab('password')}
                        className={`flex-1 px-4 py-3 text-sm font-medium transition-colors ${
                            activeTab === 'password'
                                ? 'text-primary border-b-2 border-primary bg-primary/5'
                                : 'text-muted-foreground hover:text-card-foreground'
                        }`}
                    >
                        <span className="material-symbols-outlined text-[18px] align-middle mr-2">lock</span>
                        Change Password
                    </button>
                </div>

                {/* Content */}
                <div className="p-6">
                    {error && (
                        <div className="mb-4 p-3 bg-red-50 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 rounded-lg text-red-600 dark:text-red-400 text-sm flex items-center gap-2">
                            <span className="material-symbols-outlined text-[18px]">error</span>
                            {error}
                        </div>
                    )}
                    {successMessage && (
                        <div className="mb-4 p-3 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg text-green-600 dark:text-green-400 text-sm flex items-center gap-2">
                            <span className="material-symbols-outlined text-[18px]">check_circle</span>
                            {successMessage}
                        </div>
                    )}

                    {activeTab === 'profile' ? (
                        <form onSubmit={handleProfileSubmit} className="space-y-4">
                            {/* Avatar Preview */}
                            <div className="flex items-center gap-4">
                                <div className={`size-16 rounded-full flex items-center justify-center text-white font-bold text-xl ${
                                    avatarUrl ? 'bg-slate-300' : 'bg-gradient-to-br from-blue-500 to-purple-500'
                                }`}>
                                    {avatarUrl ? (
                                        <img src={avatarUrl} alt={name} className="size-full rounded-full object-cover" />
                                    ) : (
                                        name.split(' ').map(n => n[0]).join('').toUpperCase().substring(0, 2)
                                    )}
                                </div>
                                <div className="flex-1">
                                    <p className="text-sm font-medium text-slate-700 dark:text-slate-300">Profile Picture</p>
                                    <p className="text-xs text-slate-500 dark:text-slate-400">Enter URL or leave blank for initials</p>
                                </div>
                            </div>

                            {/* Avatar URL */}
                            <div>
                                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">
                                    Avatar URL
                                </label>
                                <input
                                    type="url"
                                    value={avatarUrl}
                                    onChange={(e) => setAvatarUrl(e.target.value)}
                                    placeholder="https://example.com/avatar.jpg"
                                    className="w-full bg-slate-50 dark:bg-background-dark border border-slate-200 dark:border-border-dark text-slate-900 dark:text-slate-200 text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary placeholder-slate-400"
                                />
                            </div>

                            {/* Name */}
                            <div>
                                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">
                                    Display Name <span className="text-red-500">*</span>
                                </label>
                                <input
                                    type="text"
                                    value={name}
                                    onChange={(e) => setName(e.target.value)}
                                    required
                                    minLength={1}
                                    maxLength={100}
                                    className="w-full bg-slate-50 dark:bg-background-dark border border-slate-200 dark:border-border-dark text-slate-900 dark:text-slate-200 text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                />
                            </div>

                            {/* Email (read-only) */}
                            <div>
                                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">
                                    Email Address
                                </label>
                                <input
                                    type="email"
                                    value={user.email}
                                    disabled
                                    className="w-full bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-border-dark text-slate-500 dark:text-slate-400 text-sm rounded-lg px-4 py-2.5 cursor-not-allowed"
                                />
                                <p className="text-xs text-slate-500 mt-1">Email cannot be changed</p>
                            </div>

                            {/* Submit */}
                            <div className="flex gap-3 pt-4">
                                <button
                                    type="button"
                                    onClick={handleClose}
                                    className="flex-1 px-4 py-2.5 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 text-sm font-medium rounded-lg transition-colors"
                                >
                                    Cancel
                                </button>
                                <button
                                    type="submit"
                                    disabled={isSubmitting || !name.trim()}
                                    className="flex-1 px-4 py-2.5 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                                >
                                    {isSubmitting ? (
                                        <>
                                            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                                            Saving...
                                        </>
                                    ) : (
                                        <>
                                            <span className="material-symbols-outlined text-[18px]">save</span>
                                            Save Profile
                                        </>
                                    )}
                                </button>
                            </div>
                        </form>
                    ) : (
                        <form onSubmit={handlePasswordSubmit} className="space-y-4">
                            {/* Current Password */}
                            <div>
                                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">
                                    Current Password <span className="text-red-500">*</span>
                                </label>
                                <input
                                    type="password"
                                    value={currentPassword}
                                    onChange={(e) => setCurrentPassword(e.target.value)}
                                    required
                                    className="w-full bg-slate-50 dark:bg-background-dark border border-slate-200 dark:border-border-dark text-slate-900 dark:text-slate-200 text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                />
                            </div>

                            {/* New Password */}
                            <div>
                                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">
                                    New Password <span className="text-red-500">*</span>
                                </label>
                                <input
                                    type="password"
                                    value={newPassword}
                                    onChange={(e) => setNewPassword(e.target.value)}
                                    required
                                    minLength={8}
                                    className="w-full bg-slate-50 dark:bg-background-dark border border-slate-200 dark:border-border-dark text-slate-900 dark:text-slate-200 text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                />
                                <p className="text-xs text-slate-500 mt-1">Minimum 8 characters</p>
                            </div>

                            {/* Confirm New Password */}
                            <div>
                                <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-1">
                                    Confirm New Password <span className="text-red-500">*</span>
                                </label>
                                <input
                                    type="password"
                                    value={confirmPassword}
                                    onChange={(e) => setConfirmPassword(e.target.value)}
                                    required
                                    minLength={8}
                                    className="w-full bg-slate-50 dark:bg-background-dark border border-slate-200 dark:border-border-dark text-slate-900 dark:text-slate-200 text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                />
                            </div>

                            {/* Submit */}
                            <div className="flex gap-3 pt-4">
                                <button
                                    type="button"
                                    onClick={handleClose}
                                    className="flex-1 px-4 py-2.5 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 text-sm font-medium rounded-lg transition-colors"
                                >
                                    Cancel
                                </button>
                                <button
                                    type="submit"
                                    disabled={isSubmitting || !currentPassword || !newPassword || !confirmPassword}
                                    className="flex-1 px-4 py-2.5 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                                >
                                    {isSubmitting ? (
                                        <>
                                            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                                            Changing...
                                        </>
                                    ) : (
                                        <>
                                            <span className="material-symbols-outlined text-[18px]">lock_reset</span>
                                            Change Password
                                        </>
                                    )}
                                </button>
                            </div>
                        </form>
                    )}
                </div>
            </div>
        </div>
    );
}
