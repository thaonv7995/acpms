import { useState } from 'react';
import { createUser } from '../../api/users';
import type { SystemRole } from '../../types/user';

interface InviteUserModalProps {
    isOpen: boolean;
    onClose: () => void;
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

export function InviteUserModal({ isOpen, onClose, onSuccess }: InviteUserModalProps) {
    const [email, setEmail] = useState('');
    const [name, setName] = useState('');
    const [password, setPassword] = useState('');
    const [confirmPassword, setConfirmPassword] = useState('');
    const [selectedRoles, setSelectedRoles] = useState<SystemRole[]>(['viewer']);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Password validation rules
    const passwordRules = {
        minLength: password.length >= 8,
        hasUpperCase: /[A-Z]/.test(password),
        hasLowerCase: /[a-z]/.test(password),
        hasNumber: /[0-9]/.test(password),
        hasSpecialChar: /[!@#$%^&*()_+\-=\[\]{};':"\\|,.<>\/?]/.test(password),
    };

    const isPasswordValid = Object.values(passwordRules).every(Boolean);
    const isConfirmPasswordMatch = password === confirmPassword && confirmPassword.length > 0;
    const isFormValid = name.trim() && email.trim() && isPasswordValid && isConfirmPasswordMatch && selectedRoles.length > 0;

    const toggleRole = (role: SystemRole) => {
        setSelectedRoles(prev => {
            if (prev.includes(role)) {
                // Don't allow removing all roles
                if (prev.length === 1) return prev;
                return prev.filter(r => r !== role);
            }
            return [...prev, role];
        });
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);
        setIsSubmitting(true);

        try {
            await createUser({
                email,
                name,
                password,
                global_roles: selectedRoles,
            });

            // Reset form
            setEmail('');
            setName('');
            setPassword('');
            setConfirmPassword('');
            setSelectedRoles(['viewer']);
            onSuccess();
            onClose();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to create user');
        } finally {
            setIsSubmitting(false);
        }
    };

    const handleClose = () => {
        setEmail('');
        setName('');
        setPassword('');
        setConfirmPassword('');
        setSelectedRoles(['viewer']);
        setError(null);
        onClose();
    };

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border rounded-xl shadow-2xl w-full max-w-4xl max-h-[90vh] flex flex-col overflow-hidden">
                {/* Header */}
                <div className="flex items-center justify-between px-6 py-5 border-b border-border shrink-0">
                    <div className="flex items-center gap-3">
                        <div className="size-10 rounded-lg bg-primary/10 flex items-center justify-center">
                            <span className="material-symbols-outlined text-primary">person_add</span>
                        </div>
                        <div>
                            <h2 className="text-lg font-bold text-card-foreground">Invite New User</h2>
                            <p className="text-xs text-muted-foreground mt-0.5">Add a new user to the system</p>
                        </div>
                    </div>
                    <button
                        onClick={handleClose}
                        className="text-muted-foreground hover:text-card-foreground transition-colors p-1 hover:bg-muted rounded-lg"
                    >
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {/* Form */}
                <form onSubmit={handleSubmit} className="flex flex-col flex-1 min-h-0">
                    {/* Form Content - Scrollable */}
                    <div className="px-6 py-5 overflow-y-auto flex-1">
                        {/* Error Message */}
                        {error && (
                            <div className="mb-4 p-3 bg-red-50 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 rounded-lg text-red-600 dark:text-red-400 text-sm flex items-start gap-2">
                                <span className="material-symbols-outlined text-[18px] shrink-0">error</span>
                                <span className="flex-1">{error}</span>
                            </div>
                        )}

                        {/* Horizontal Layout: Left (Form Fields) + Right (Roles) */}
                        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                            {/* Left Column: Basic Info & Security */}
                            <div className="space-y-6">
                                {/* Basic Information Section */}
                                <div className="space-y-4">
                                    <div className="flex items-center gap-2 pb-2 border-b border-border">
                                        <span className="material-symbols-outlined text-primary text-[18px]">person</span>
                                        <h3 className="text-sm font-bold text-card-foreground uppercase tracking-wide">Basic Information</h3>
                                    </div>

                                    {/* Full Name */}
                                    <div>
                                        <label className="block text-xs font-medium text-card-foreground mb-1.5">
                                            Full Name <span className="text-red-500">*</span>
                                        </label>
                                        <input
                                            type="text"
                                            value={name}
                                            onChange={(e) => setName(e.target.value)}
                                            placeholder="John Doe"
                                            required
                                            className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg px-3 py-2 focus:ring-1 focus:ring-primary focus:border-primary placeholder-muted-foreground"
                                        />
                                    </div>

                                    {/* Email Address */}
                                    <div>
                                        <label className="block text-xs font-medium text-card-foreground mb-1.5">
                                            Email Address <span className="text-red-500">*</span>
                                        </label>
                                        <input
                                            type="email"
                                            value={email}
                                            onChange={(e) => setEmail(e.target.value)}
                                            placeholder="john@company.com"
                                            required
                                            className="w-full bg-muted border border-border text-card-foreground text-sm rounded-lg px-3 py-2 focus:ring-1 focus:ring-primary focus:border-primary placeholder-muted-foreground"
                                        />
                                    </div>
                                </div>

                                {/* Security Section */}
                                <div className="space-y-4">
                                    <div className="flex items-center gap-2 pb-2 border-b border-border">
                                        <span className="material-symbols-outlined text-primary text-[18px]">lock</span>
                                        <h3 className="text-sm font-bold text-card-foreground uppercase tracking-wide">Security</h3>
                                    </div>

                                    {/* Initial Password */}
                                    <div>
                                        <label className="block text-xs font-medium text-card-foreground mb-1.5">
                                            Initial Password <span className="text-red-500">*</span>
                                        </label>
                                        <input
                                            type="password"
                                            value={password}
                                            onChange={(e) => setPassword(e.target.value)}
                                            placeholder="Enter a strong password"
                                            required
                                            className={`w-full bg-muted border text-card-foreground text-sm rounded-lg px-3 py-2 focus:ring-1 focus:ring-primary focus:border-primary placeholder-muted-foreground ${
                                                password.length > 0 && !isPasswordValid
                                                    ? 'border-red-500 dark:border-red-500'
                                                    : password.length > 0 && isPasswordValid
                                                    ? 'border-green-500 dark:border-green-500'
                                                    : 'border-border'
                                            }`}
                                        />
                                        
                                        {/* Password Rules */}
                                        {password.length > 0 && (
                                            <div className="mt-2 space-y-1.5">
                                                <p className="text-xs font-medium text-card-foreground mb-1.5">Password must contain:</p>
                                                <div className="space-y-1">
                                                    <div className={`flex items-center gap-2 text-xs ${passwordRules.minLength ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'}`}>
                                                        <span className="material-symbols-outlined text-[16px]">
                                                            {passwordRules.minLength ? 'check_circle' : 'radio_button_unchecked'}
                                                        </span>
                                                        <span>At least 8 characters</span>
                                                    </div>
                                                    <div className={`flex items-center gap-2 text-xs ${passwordRules.hasUpperCase ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'}`}>
                                                        <span className="material-symbols-outlined text-[16px]">
                                                            {passwordRules.hasUpperCase ? 'check_circle' : 'radio_button_unchecked'}
                                                        </span>
                                                        <span>One uppercase letter (A-Z)</span>
                                                    </div>
                                                    <div className={`flex items-center gap-2 text-xs ${passwordRules.hasLowerCase ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'}`}>
                                                        <span className="material-symbols-outlined text-[16px]">
                                                            {passwordRules.hasLowerCase ? 'check_circle' : 'radio_button_unchecked'}
                                                        </span>
                                                        <span>One lowercase letter (a-z)</span>
                                                    </div>
                                                    <div className={`flex items-center gap-2 text-xs ${passwordRules.hasNumber ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'}`}>
                                                        <span className="material-symbols-outlined text-[16px]">
                                                            {passwordRules.hasNumber ? 'check_circle' : 'radio_button_unchecked'}
                                                        </span>
                                                        <span>One number (0-9)</span>
                                                    </div>
                                                    <div className={`flex items-center gap-2 text-xs ${passwordRules.hasSpecialChar ? 'text-green-600 dark:text-green-400' : 'text-muted-foreground'}`}>
                                                        <span className="material-symbols-outlined text-[16px]">
                                                            {passwordRules.hasSpecialChar ? 'check_circle' : 'radio_button_unchecked'}
                                                        </span>
                                                        <span>One special character (!@#$%^&*)</span>
                                                    </div>
                                                </div>
                                            </div>
                                        )}
                                    </div>

                                    {/* Confirm Password */}
                                    <div>
                                        <label className="block text-xs font-medium text-card-foreground mb-1.5">
                                            Confirm Password <span className="text-red-500">*</span>
                                        </label>
                                        <input
                                            type="password"
                                            value={confirmPassword}
                                            onChange={(e) => setConfirmPassword(e.target.value)}
                                            placeholder="Re-enter password"
                                            required
                                            className={`w-full bg-muted border text-card-foreground text-sm rounded-lg px-3 py-2 focus:ring-1 focus:ring-primary focus:border-primary placeholder-muted-foreground ${
                                                confirmPassword.length > 0 && !isConfirmPasswordMatch
                                                    ? 'border-red-500 dark:border-red-500'
                                                    : confirmPassword.length > 0 && isConfirmPasswordMatch
                                                    ? 'border-green-500 dark:border-green-500'
                                                    : 'border-border'
                                            }`}
                                        />
                                        {confirmPassword.length > 0 && !isConfirmPasswordMatch && (
                                            <p className="text-xs text-red-500 dark:text-red-400 mt-1.5 flex items-center gap-1">
                                                <span className="material-symbols-outlined text-[14px]">error</span>
                                                Passwords do not match
                                            </p>
                                        )}
                                        {confirmPassword.length > 0 && isConfirmPasswordMatch && (
                                            <p className="text-xs text-green-600 dark:text-green-400 mt-1.5 flex items-center gap-1">
                                                <span className="material-symbols-outlined text-[14px]">check_circle</span>
                                                Passwords match
                                            </p>
                                        )}
                                    </div>

                                    <p className="text-xs text-muted-foreground pt-2 border-t border-border">
                                        User can change password after first login
                                    </p>
                                </div>
                            </div>

                            {/* Right Column: Global Roles */}
                            <div className="space-y-4">
                                <div className="flex items-center gap-2 pb-2 border-b border-border">
                                    <span className="material-symbols-outlined text-primary text-[18px]">badge</span>
                                    <h3 className="text-sm font-bold text-card-foreground uppercase tracking-wide">Global Roles</h3>
                                    <span className="text-red-500 text-xs">*</span>
                                </div>

                                <div className="space-y-2 max-h-[400px] overflow-y-auto pr-1">
                                    {AVAILABLE_ROLES.map((role) => (
                                        <label
                                            key={role.value}
                                            className={`flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-all ${
                                                selectedRoles.includes(role.value)
                                                    ? 'bg-primary/10 dark:bg-primary/20 border-primary'
                                                    : 'bg-muted border-border hover:border-border/80 hover:bg-muted/80'
                                            }`}
                                        >
                                            <input
                                                type="checkbox"
                                                checked={selectedRoles.includes(role.value)}
                                                onChange={() => toggleRole(role.value)}
                                                className="mt-0.5 size-4 rounded border-border text-primary focus:ring-primary focus:ring-offset-0 shrink-0"
                                            />
                                            <div className="flex-1 min-w-0">
                                                <p className="text-sm font-medium text-card-foreground">{role.label}</p>
                                                <p className="text-xs text-muted-foreground mt-0.5 leading-relaxed">{role.description}</p>
                                            </div>
                                        </label>
                                    ))}
                                </div>
                                <p className="text-xs text-muted-foreground">Select at least one role. Project-specific roles can be assigned later.</p>
                            </div>
                        </div>
                    </div>

                    {/* Footer */}
                    <div className="px-6 py-4 border-t border-border bg-muted shrink-0">
                        <div className="flex gap-3 justify-end">
                            <button
                                type="button"
                                onClick={handleClose}
                                className="px-4 py-2 bg-card border border-border hover:bg-muted text-card-foreground text-sm font-medium rounded-lg transition-colors"
                            >
                                Cancel
                            </button>
                            <button
                                type="submit"
                                disabled={isSubmitting || !isFormValid}
                                className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                            >
                                {isSubmitting ? (
                                    <>
                                        <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-white"></div>
                                        Creating...
                                    </>
                                ) : (
                                    <>
                                        <span className="material-symbols-outlined text-[18px]">send</span>
                                        Create User
                                    </>
                                )}
                            </button>
                        </div>
                    </div>
                </form>
            </div>
        </div>
    );
}
