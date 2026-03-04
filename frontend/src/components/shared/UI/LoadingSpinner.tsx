interface LoadingSpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  variant?: 'primary' | 'white' | 'slate';
}

const sizeClasses = {
  sm: 'w-4 h-4 border-2',
  md: 'w-6 h-6 border-2',
  lg: 'w-8 h-8 border-3',
};

const variantClasses = {
  primary: 'border-primary border-t-transparent',
  white: 'border-white border-t-transparent',
  slate: 'border-slate-300 dark:border-slate-600 border-t-transparent',
};

export function LoadingSpinner({ size = 'md', variant = 'primary' }: LoadingSpinnerProps) {
  return (
    <div
      className={`
        inline-block rounded-full animate-spin
        ${sizeClasses[size]}
        ${variantClasses[variant]}
      `}
      role="status"
      aria-label="Loading"
    >
      <span className="sr-only">Loading...</span>
    </div>
  );
}
