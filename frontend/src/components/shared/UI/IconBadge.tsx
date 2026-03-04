interface IconBadgeProps {
  icon: string;
  variant?: 'primary' | 'success' | 'warning' | 'danger' | 'info' | 'neutral';
  size?: 'sm' | 'md' | 'lg';
}

const variantClasses = {
  primary: 'bg-primary/10 text-primary',
  success: 'bg-green-100 dark:bg-green-900/30 text-green-600',
  warning: 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-600',
  danger: 'bg-red-100 dark:bg-red-900/30 text-red-600',
  info: 'bg-blue-100 dark:bg-blue-900/30 text-blue-600',
  neutral: 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400',
};

const sizeClasses = {
  sm: 'w-8 h-8 text-base',
  md: 'w-10 h-10 text-lg',
  lg: 'w-12 h-12 text-xl',
};

export function IconBadge({ icon, variant = 'neutral', size = 'md' }: IconBadgeProps) {
  return (
    <div
      className={`
        flex items-center justify-center rounded-lg
        ${variantClasses[variant]}
        ${sizeClasses[size]}
      `}
    >
      <span className="material-symbols-outlined">{icon}</span>
    </div>
  );
}
