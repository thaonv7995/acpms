import { ReactNode } from 'react';

interface DropdownItemProps {
  icon?: string;
  label: string;
  onClick?: () => void;
  variant?: 'default' | 'danger';
  disabled?: boolean;
  children?: ReactNode;
}

export function DropdownItem({
  icon,
  label,
  onClick,
  variant = 'default',
  disabled = false,
  children,
}: DropdownItemProps) {
  const variantClasses = {
    default: 'text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800',
    danger: 'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20',
  };

  if (children) {
    return <div className="px-1">{children}</div>;
  }

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`
        w-full flex items-center gap-3 px-4 py-2 text-sm text-left
        transition-colors
        ${variantClasses[variant]}
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
      `}
      role="menuitem"
    >
      {icon && (
        <span className="material-symbols-outlined text-base">{icon}</span>
      )}
      <span>{label}</span>
    </button>
  );
}
