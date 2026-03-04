import React from 'react';

interface ModalFooterProps {
  onCancel?: () => void;
  onConfirm?: () => void;
  cancelText?: string;
  confirmText?: string;
  confirmVariant?: 'primary' | 'danger' | 'success';
  isLoading?: boolean;
  children?: React.ReactNode;
}

const variantClasses = {
  primary: 'bg-primary hover:bg-primary/90 shadow-primary/20',
  danger: 'bg-red-600 hover:bg-red-700 shadow-red-600/20',
  success: 'bg-green-600 hover:bg-green-700 shadow-green-600/20',
};

export function ModalFooter({
  onCancel,
  onConfirm,
  cancelText = 'Cancel',
  confirmText = 'Confirm',
  confirmVariant = 'primary',
  isLoading = false,
  children,
}: ModalFooterProps) {
  if (children) {
    return <div className="flex justify-end gap-3 mt-6">{children}</div>;
  }

  return (
    <div className="flex justify-end gap-3 mt-6">
      {onCancel && (
        <button
          onClick={onCancel}
          disabled={isLoading}
          className="px-4 py-2 text-sm font-medium text-slate-600 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white transition-colors disabled:opacity-50"
        >
          {cancelText}
        </button>
      )}
      {onConfirm && (
        <button
          onClick={onConfirm}
          disabled={isLoading}
          className={`px-5 py-2 text-white text-sm font-bold rounded-lg shadow-lg transition-all disabled:opacity-50 ${variantClasses[confirmVariant]}`}
        >
          {isLoading ? 'Loading...' : confirmText}
        </button>
      )}
    </div>
  );
}
