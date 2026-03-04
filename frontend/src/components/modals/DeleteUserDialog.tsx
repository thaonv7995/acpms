// DeleteUserDialog Component
import type { User } from '../../api/users';

interface DeleteUserDialogProps {
    isOpen: boolean;
    onClose: () => void;
    user: User | null;
    onConfirm: (userId: string) => Promise<void>;
    isDeleting?: boolean;
}

export function DeleteUserDialog({
    isOpen,
    onClose,
    user,
    onConfirm,
    isDeleting = false,
}: DeleteUserDialogProps) {
    if (!isOpen || !user) return null;

    const handleConfirm = () => {
        onConfirm(user.id);
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose}></div>
            <div className="relative w-full max-w-md bg-white dark:bg-[#0d1117] border border-slate-200 dark:border-slate-700 rounded-2xl shadow-2xl overflow-hidden">
                <div className="p-6">
                    {/* Icon and Title */}
                    <div className="flex items-center gap-3 mb-4">
                        <div className="p-2 rounded-lg bg-red-100 dark:bg-red-900/30 text-red-600">
                            <span className="material-symbols-outlined text-2xl">warning</span>
                        </div>
                        <h2 className="text-lg font-bold text-slate-900 dark:text-white">Delete User</h2>
                    </div>

                    {/* Message */}
                    <div className="mb-6 space-y-3">
                        <p className="text-sm text-slate-600 dark:text-slate-400">
                            Are you sure you want to delete this user? This action cannot be undone.
                        </p>

                        {/* User Info Card */}
                        <div className="bg-slate-50 dark:bg-[#161b22] border border-slate-200 dark:border-slate-700 rounded-lg p-4">
                            <div className="flex items-center gap-3">
                                {/* Avatar */}
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

                                {/* User Details */}
                                <div className="flex-1">
                                    <p className="font-bold text-slate-900 dark:text-white">{user.name}</p>
                                    <p className="text-xs text-slate-500 dark:text-slate-400">{user.email}</p>
                                </div>

                                {/* Role Badges */}
                                <div className="flex gap-1">
                                    {user.globalRoles.map((role) => (
                                        <span
                                            key={role}
                                            className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${
                                                role === 'admin'
                                                    ? 'bg-purple-100 dark:bg-purple-500/10 text-purple-700 dark:text-purple-400'
                                                    : 'bg-blue-100 dark:bg-blue-500/10 text-blue-700 dark:text-blue-400'
                                            }`}
                                        >
                                            {role}
                                        </span>
                                    ))}
                                </div>
                            </div>
                        </div>

                        {/* Warning */}
                        <div className="flex items-start gap-2 px-3 py-2 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
                            <span className="material-symbols-outlined text-amber-600 dark:text-amber-400 text-[18px] mt-0.5">info</span>
                            <p className="text-xs text-amber-700 dark:text-amber-300">
                                All projects, tasks, and activity history associated with this user will be preserved but marked as deleted.
                            </p>
                        </div>
                    </div>

                    {/* Actions */}
                    <div className="flex justify-end gap-3">
                        <button
                            onClick={onClose}
                            disabled={isDeleting}
                            className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white transition-colors disabled:opacity-50"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={handleConfirm}
                            disabled={isDeleting}
                            className="px-5 py-2 bg-red-600 hover:bg-red-700 text-white text-sm font-bold rounded-lg shadow-lg shadow-red-600/20 transition-all disabled:opacity-50 flex items-center gap-2"
                        >
                            {isDeleting ? (
                                <>
                                    <span className="animate-spin material-symbols-outlined text-[18px]">progress_activity</span>
                                    Deleting...
                                </>
                            ) : (
                                <>
                                    <span className="material-symbols-outlined text-[18px]">delete</span>
                                    Delete User
                                </>
                            )}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
