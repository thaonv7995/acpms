// Toast notification component
import { useEffect } from 'react';

export interface ToastProps {
    message: string;
    type?: 'success' | 'error' | 'info' | 'warning';
    onClose: () => void;
    duration?: number;
}

const typeStyles = {
    success: {
        bg: 'bg-green-100 dark:bg-green-900/50',
        border: 'border-green-200 dark:border-green-800',
        text: 'text-green-700 dark:text-green-300',
        icon: 'check_circle',
        iconColor: 'text-green-500',
    },
    error: {
        bg: 'bg-red-100 dark:bg-red-900/50',
        border: 'border-red-200 dark:border-red-800',
        text: 'text-red-700 dark:text-red-300',
        icon: 'error',
        iconColor: 'text-red-500',
    },
    info: {
        bg: 'bg-blue-100 dark:bg-blue-900/50',
        border: 'border-blue-200 dark:border-blue-800',
        text: 'text-blue-700 dark:text-blue-300',
        icon: 'info',
        iconColor: 'text-blue-500',
    },
    warning: {
        bg: 'bg-amber-100 dark:bg-amber-900/50',
        border: 'border-amber-200 dark:border-amber-800',
        text: 'text-amber-700 dark:text-amber-300',
        icon: 'warning',
        iconColor: 'text-amber-500',
    },
};

export function Toast({ message, type = 'success', onClose, duration = 3000 }: ToastProps) {
    const styles = typeStyles[type];

    useEffect(() => {
        if (duration > 0) {
            const timer = setTimeout(onClose, duration);
            return () => clearTimeout(timer);
        }
    }, [duration, onClose]);

    return (
        <div className={`fixed top-4 right-4 z-50 ${styles.bg} border ${styles.border} ${styles.text} px-4 py-3 rounded-lg shadow-lg flex items-center gap-2 animate-fade-in`}>
            <span className={`material-symbols-outlined ${styles.iconColor}`}>{styles.icon}</span>
            <span>{message}</span>
            <button
                onClick={onClose}
                className="ml-2 text-current hover:opacity-70 transition-opacity"
            >
                <span className="material-symbols-outlined text-[18px]">close</span>
            </button>
        </div>
    );
}
