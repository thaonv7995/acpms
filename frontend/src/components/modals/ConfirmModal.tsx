import { logger } from '@/lib/logger';
// ConfirmModal Component
interface ConfirmModalProps {
    isOpen: boolean;
    onClose: () => void;
    onConfirm: () => Promise<void> | void;
    title: string;
    message: string;
    confirmText?: string;
    confirmVariant?: 'danger' | 'primary';
    isLoading?: boolean;
}

export function ConfirmModal({
    isOpen,
    onClose,
    onConfirm,
    title,
    message,
    confirmText = 'Confirm',
    confirmVariant = 'danger',
    isLoading = false,
}: ConfirmModalProps) {
    if (!isOpen) return null;

    const confirmClasses = confirmVariant === 'danger'
        ? 'bg-red-600 hover:bg-red-700 shadow-red-600/20'
        : 'bg-primary hover:bg-primary/90 shadow-primary/20';

    const handleConfirm = async () => {
        try {
            await onConfirm();
        } catch (error) {
            logger.error('Confirmation action failed:', error);
        }
    };

    return (
        <div
            className="fixed inset-0 z-[60] flex items-center justify-center p-4 font-display"
            role="dialog"
            aria-modal="true"
            aria-labelledby="confirm-modal-title"
            aria-describedby="confirm-modal-message"
        >
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose}></div>
            <div className="relative w-full max-w-md bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
                <div className="p-6">
                    <div className="flex items-center gap-3 mb-4">
                        <div className={`p-2 rounded-lg ${confirmVariant === 'danger' ? 'bg-red-100 dark:bg-red-500/20 text-red-600 dark:text-red-400' : 'bg-primary/10 text-primary'}`}>
                            <span className="material-symbols-outlined">
                                {confirmVariant === 'danger' ? 'warning' : 'help'}
                            </span>
                        </div>
                        <h2 id="confirm-modal-title" className="text-lg font-bold text-card-foreground">{title}</h2>
                    </div>
                    <p id="confirm-modal-message" className="text-sm text-muted-foreground mb-6">{message}</p>
                    <div className="flex justify-end gap-3">
                        <button
                            onClick={onClose}
                            disabled={isLoading}
                            className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors disabled:opacity-50"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={handleConfirm}
                            disabled={isLoading}
                            className={`px-5 py-2 text-white text-sm font-bold rounded-lg shadow-lg transition-all disabled:opacity-50 ${confirmClasses}`}
                        >
                            {isLoading ? 'Loading...' : confirmText}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
