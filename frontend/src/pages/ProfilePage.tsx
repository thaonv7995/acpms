import { useState, useEffect, useMemo } from 'react';
import { AppShell } from '../components/layout/AppShell';
import { getCurrentUser, setCurrentUser } from '../api/auth';
import { apiPut, apiPost } from '../api/client';
import type { UserDto } from '../api/generated/models';
import { logger } from '@/lib/logger';

export function ProfilePage() {
    const currentUser = useMemo(() => getCurrentUser(), []);

    // Profile Info state
    const [name, setName] = useState('');
    const [avatarUrl, setAvatarUrl] = useState(''); // S3 key for save, or presigned URL from server
    const [avatarPreviewUrl, setAvatarPreviewUrl] = useState<string | null>(null); // Object URL for immediate preview after upload

    // Password state
    const [currentPassword, setCurrentPassword] = useState('');
    const [newPassword, setNewPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');

    // UI state
    const [isSubmittingProfile, setIsSubmittingProfile] = useState(false);
    const [isSubmittingPassword, setIsSubmittingPassword] = useState(false);
    const [profileMessage, setProfileMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
    const [passwordMessage, setPasswordMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
    const [isUploadingAvatar, setIsUploadingAvatar] = useState(false);

    useEffect(() => {
        if (currentUser) {
            setName(currentUser.name);
            setAvatarUrl(currentUser.avatar_url || '');
            setAvatarPreviewUrl(null); // Server URL used for display via avatarUrl
        }
    }, [currentUser]);

    // Revoke object URL on unmount to avoid memory leak
    useEffect(() => {
        return () => {
            if (avatarPreviewUrl) URL.revokeObjectURL(avatarPreviewUrl);
        };
    }, [avatarPreviewUrl]);

    const handleProfileSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!currentUser) return;

        setProfileMessage(null);
        setIsSubmittingProfile(true);

        try {
            const updatedUser = await apiPut<UserDto>(`/api/v1/users/${currentUser.id}`, {
                name: name.trim(),
                avatar_url: avatarUrl.trim() || null,
            });
            setCurrentUser(updatedUser);
            setAvatarUrl(updatedUser.avatar_url || '');
            setAvatarPreviewUrl((prev) => {
                if (prev) URL.revokeObjectURL(prev);
                return null;
            });
            setProfileMessage({ type: 'success', text: 'Profile updated successfully!' });
        } catch (err) {
            setProfileMessage({ type: 'error', text: err instanceof Error ? err.message : 'Failed to update profile' });
        } finally {
            setIsSubmittingProfile(false);
        }
    };

    const handlePasswordSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!currentUser) return;

        setPasswordMessage(null);

        // Validation
        if (!currentPassword) {
            setPasswordMessage({ type: 'error', text: 'Current password is required' });
            return;
        }
        if (newPassword.length < 8) {
            setPasswordMessage({ type: 'error', text: 'New password must be at least 8 characters' });
            return;
        }
        if (newPassword !== confirmPassword) {
            setPasswordMessage({ type: 'error', text: 'New passwords do not match' });
            return;
        }

        setIsSubmittingPassword(true);

        try {
            await apiPut(`/api/v1/users/${currentUser.id}/password`, {
                current_password: currentPassword,
                new_password: newPassword,
            });
            setPasswordMessage({ type: 'success', text: 'Password changed successfully!' });
            setCurrentPassword('');
            setNewPassword('');
            setConfirmPassword('');
        } catch (err) {
            setPasswordMessage({ type: 'error', text: err instanceof Error ? err.message : 'Failed to change password' });
        } finally {
            setIsSubmittingPassword(false);
        }
    };

    const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;

        // Revoke previous object URL to avoid memory leak
        setAvatarPreviewUrl((prev) => {
            if (prev) URL.revokeObjectURL(prev);
            return null;
        });

        setIsUploadingAvatar(true);
        try {
            const contentType = file.type || 'image/jpeg';
            // 1. Get Presigned URL
            const { upload_url, key } = await apiPost<{ upload_url: string; key: string }>(
                '/api/v1/users/avatar/upload-url',
                { filename: file.name, content_type: contentType }
            );

            // 2. Upload to S3
            const uploadRes = await fetch(upload_url, {
                method: 'PUT',
                body: file,
                headers: { 'Content-Type': contentType },
            });

            if (!uploadRes.ok) {
                throw new Error('Failed to upload image to storage');
            }

            // 3. Store S3 key for save; use object URL for immediate preview (key is not a valid img src)
            setAvatarUrl(key);
            setAvatarPreviewUrl(URL.createObjectURL(file));
            setProfileMessage({ type: 'success', text: 'Image uploaded! Click Save to apply.' });
        } catch (err) {
            logger.error(err);
            setProfileMessage({ type: 'error', text: 'Avatar upload failed' });
        } finally {
            setIsUploadingAvatar(false);
        }
    };

    if (!currentUser) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center">
                    <p className="text-muted-foreground">Please login to view your profile</p>
                </div>
            </AppShell>
        );
    }

    const avatarInitials = name.split(' ').map(n => n[0]).join('').toUpperCase().substring(0, 2);

    // File input ref
    const fileInputRef = useMemo(() => {
        return { current: null as HTMLInputElement | null };
    }, []);

    const triggerFileUpload = () => {
        fileInputRef.current?.click();
    };

    return (
        <AppShell>
            <div className="flex-1 overflow-y-auto p-6 md:p-8 scrollbar-hide bg-background">
                <div className="max-w-3xl mx-auto">
                    {/* Header */}
                    <div className="mb-8">
                        <h1 className="text-3xl font-bold text-card-foreground mb-2">My Profile</h1>
                        <p className="text-muted-foreground text-sm">
                            Manage your account settings and change your password
                        </p>
                    </div>

                    {/* Profile Card */}
                    <div className="bg-card border border-border rounded-xl shadow-sm mb-6">
                        {/* Profile Header */}
                        <div className="p-6 border-b border-border">
                            <div className="flex items-center gap-4">
                                <div
                                    onClick={triggerFileUpload}
                                    className={`size-20 rounded-full flex items-center justify-center text-white font-bold text-2xl relative group cursor-pointer overflow-hidden transition-transform active:scale-95 ${(avatarPreviewUrl || avatarUrl) ? 'bg-slate-300' : 'bg-gradient-to-br from-blue-500 to-purple-500'
                                        }`}>
                                    {/* Avatar Image or Initials: avatarPreviewUrl (object URL) for pending upload, avatarUrl (presigned) from server */}
                                    {avatarPreviewUrl || avatarUrl ? (
                                        <img src={avatarPreviewUrl || avatarUrl} alt={name} className="size-full rounded-full object-cover" />
                                    ) : (
                                        avatarInitials
                                    )}

                                    {/* Hover Overlay */}
                                    <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                                        <span className="material-symbols-outlined text-white text-3xl">photo_camera</span>
                                    </div>

                                    {/* Loading Overlay */}
                                    {isUploadingAvatar && (
                                        <div className="absolute inset-0 bg-black/50 flex items-center justify-center">
                                            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-white"></div>
                                        </div>
                                    )}
                                </div>
                                {/* Hidden File Input */}
                                <input
                                    type="file"
                                    ref={(e) => { if (fileInputRef) fileInputRef.current = e; }}
                                    accept="image/*"
                                    onChange={handleFileChange}
                                    className="hidden"
                                />
                                <div>
                                    <h2 className="text-xl font-bold text-card-foreground">{currentUser.name}</h2>
                                    <p className="text-muted-foreground">{currentUser.email}</p>
                                    <div className="flex gap-1 mt-2">
                                        {currentUser.global_roles.map((role) => {
                                            const roleColors: Record<string, string> = {
                                                admin: 'bg-purple-100 dark:bg-purple-500/20 text-purple-700 dark:text-purple-400 border-purple-200 dark:border-purple-500/30',
                                                product_owner: 'bg-orange-100 dark:bg-orange-500/20 text-orange-700 dark:text-orange-400 border-orange-200 dark:border-orange-500/30',
                                                business_analyst: 'bg-cyan-100 dark:bg-cyan-500/20 text-cyan-700 dark:text-cyan-400 border-cyan-200 dark:border-cyan-500/30',
                                                developer: 'bg-green-100 dark:bg-green-500/20 text-green-700 dark:text-green-400 border-green-200 dark:border-green-500/30',
                                                quality_assurance: 'bg-pink-100 dark:bg-pink-500/20 text-pink-700 dark:text-pink-400 border-pink-200 dark:border-pink-500/30',
                                                viewer: 'bg-muted border-border text-muted-foreground',
                                            };
                                            const displayNames: Record<string, string> = {
                                                admin: 'Admin', product_owner: 'PO', business_analyst: 'BA',
                                                developer: 'Dev', quality_assurance: 'QA', viewer: 'Viewer'
                                            };
                                            return (
                                                <span
                                                    key={role}
                                                    className={`px-2 py-0.5 rounded-full text-xs font-medium border ${roleColors[role] || roleColors.viewer}`}
                                                >
                                                    {displayNames[role] || role}
                                                </span>
                                            );
                                        })}
                                    </div>
                                </div>
                            </div>
                        </div>

                        {/* Profile Form */}
                        <form onSubmit={handleProfileSubmit} className="p-6">
                            <h3 className="text-lg font-semibold text-card-foreground mb-4 flex items-center gap-2">
                                <span className="material-symbols-outlined text-primary">badge</span>
                                Profile Information
                            </h3>

                            {profileMessage && (
                                <div className={`mb-4 p-3 rounded-lg text-sm flex items-center gap-2 ${profileMessage.type === 'success'
                                        ? 'bg-green-50 dark:bg-green-500/20 border border-green-200 dark:border-green-500/30 text-green-600 dark:text-green-400'
                                        : 'bg-red-50 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 text-red-600 dark:text-red-400'
                                    }`}>
                                    <span className="material-symbols-outlined text-[18px]">
                                        {profileMessage.type === 'success' ? 'check_circle' : 'error'}
                                    </span>
                                    {profileMessage.text}
                                </div>
                            )}

                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <div>
                                    <label className="block text-sm font-medium text-card-foreground mb-1">
                                        Display Name <span className="text-red-500">*</span>
                                    </label>
                                    <input
                                        type="text"
                                        value={name}
                                        onChange={(e) => setName(e.target.value)}
                                        required
                                        className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                    />
                                </div>
                                <div>
                                    <label className="block text-sm font-medium text-card-foreground mb-1">
                                        Email Address
                                    </label>
                                    <input
                                        type="email"
                                        value={currentUser.email}
                                        disabled
                                        className="w-full bg-muted/50 border border-border text-muted-foreground text-sm rounded-lg px-4 py-2.5 cursor-not-allowed"
                                    />
                                </div>
                            </div>

                            <div className="mt-6 flex justify-end">
                                <button
                                    type="submit"
                                    disabled={isSubmittingProfile || !name.trim()}
                                    className="px-6 py-2.5 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                                >
                                    {isSubmittingProfile ? (
                                        <>
                                            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
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

                    {/* Password Card */}
                    <div className="bg-card border border-border rounded-xl shadow-sm">
                        <form onSubmit={handlePasswordSubmit} className="p-6">
                            <h3 className="text-lg font-semibold text-card-foreground mb-4 flex items-center gap-2">
                                <span className="material-symbols-outlined text-primary">lock</span>
                                Change Password
                            </h3>

                            {passwordMessage && (
                                <div className={`mb-4 p-3 rounded-lg text-sm flex items-center gap-2 ${passwordMessage.type === 'success'
                                        ? 'bg-green-50 dark:bg-green-500/20 border border-green-200 dark:border-green-500/30 text-green-600 dark:text-green-400'
                                        : 'bg-red-50 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 text-red-600 dark:text-red-400'
                                    }`}>
                                    <span className="material-symbols-outlined text-[18px]">
                                        {passwordMessage.type === 'success' ? 'check_circle' : 'error'}
                                    </span>
                                    {passwordMessage.text}
                                </div>
                            )}

                            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                                <div>
                                    <label className="block text-sm font-medium text-card-foreground mb-1">
                                        Current Password <span className="text-red-500">*</span>
                                    </label>
                                    <input
                                        type="password"
                                        value={currentPassword}
                                        onChange={(e) => setCurrentPassword(e.target.value)}
                                        className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                    />
                                </div>
                                <div>
                                    <label className="block text-sm font-medium text-card-foreground mb-1">
                                        New Password <span className="text-red-500">*</span>
                                    </label>
                                    <input
                                        type="password"
                                        value={newPassword}
                                        onChange={(e) => setNewPassword(e.target.value)}
                                        className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                    />
                                    <p className="text-xs text-muted-foreground mt-1">Min. 8 characters</p>
                                </div>
                                <div>
                                    <label className="block text-sm font-medium text-card-foreground mb-1">
                                        Confirm Password <span className="text-red-500">*</span>
                                    </label>
                                    <input
                                        type="password"
                                        value={confirmPassword}
                                        onChange={(e) => setConfirmPassword(e.target.value)}
                                        className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg px-4 py-2.5 focus:ring-1 focus:ring-primary focus:border-primary"
                                    />
                                </div>
                            </div>

                            <div className="mt-6 flex justify-end">
                                <button
                                    type="submit"
                                    disabled={isSubmittingPassword || !currentPassword || !newPassword || !confirmPassword}
                                    className="px-6 py-2.5 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                                >
                                    {isSubmittingPassword ? (
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
                    </div>

                    {/* Account Info */}
                    <div className="mt-6 p-4 bg-muted rounded-lg border border-border">
                        <p className="text-sm text-muted-foreground">
                            <span className="font-medium">Account created:</span> {new Date(currentUser.created_at).toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}
                        </p>
                    </div>
                </div>
            </div>
        </AppShell>
    );
}
