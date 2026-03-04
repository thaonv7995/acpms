interface StatusBadgeProps {
  status: 'success' | 'warning' | 'error' | 'info' | 'neutral';
  label: string;
  showDot?: boolean;
  size?: 'sm' | 'md';
}

const statusStyles = {
  success: {
    bg: 'bg-green-100 dark:bg-green-900/20',
    text: 'text-green-700 dark:text-green-400',
    dot: 'bg-green-500',
  },
  warning: {
    bg: 'bg-yellow-100 dark:bg-yellow-900/20',
    text: 'text-yellow-700 dark:text-yellow-400',
    dot: 'bg-yellow-500',
  },
  error: {
    bg: 'bg-red-100 dark:bg-red-900/20',
    text: 'text-red-700 dark:text-red-400',
    dot: 'bg-red-500',
  },
  info: {
    bg: 'bg-blue-100 dark:bg-blue-900/20',
    text: 'text-blue-700 dark:text-blue-400',
    dot: 'bg-blue-500',
  },
  neutral: {
    bg: 'bg-slate-100 dark:bg-slate-800',
    text: 'text-slate-700 dark:text-slate-300',
    dot: 'bg-slate-500',
  },
};

const sizeClasses = {
  sm: 'px-2 py-0.5 text-xs',
  md: 'px-2.5 py-1 text-sm',
};

export function StatusBadge({ status, label, showDot = true, size = 'md' }: StatusBadgeProps) {
  const styles = statusStyles[status];

  return (
    <span
      className={`
        inline-flex items-center gap-1.5 rounded-full font-medium
        ${styles.bg} ${styles.text} ${sizeClasses[size]}
      `}
    >
      {showDot && <span className={`w-1.5 h-1.5 rounded-full ${styles.dot}`} />}
      {label}
    </span>
  );
}
