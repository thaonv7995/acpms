import React from 'react';

interface FormTextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
  helperText?: string;
  fullWidth?: boolean;
}

export const FormTextarea = React.forwardRef<HTMLTextAreaElement, FormTextareaProps>(
  ({ label, error, helperText, fullWidth = true, className = '', ...props }, ref) => {
    const textareaClasses = `
      px-4 py-2.5 rounded-lg
      bg-slate-50 dark:bg-[#0d1117]
      border ${error ? 'border-red-500' : 'border-slate-300 dark:border-slate-700'}
      text-slate-900 dark:text-white
      placeholder:text-slate-400 dark:placeholder:text-slate-500
      focus:outline-none focus:ring-2 focus:ring-primary/50
      disabled:opacity-50 disabled:cursor-not-allowed
      transition-colors
      resize-y
      ${fullWidth ? 'w-full' : ''}
      ${className}
    `.trim().replace(/\s+/g, ' ');

    return (
      <div className={fullWidth ? 'w-full' : ''}>
        {label && (
          <label className="block text-sm font-medium text-slate-700 dark:text-slate-300 mb-2">
            {label}
            {props.required && <span className="text-red-500 ml-1">*</span>}
          </label>
        )}
        <textarea
          ref={ref}
          className={textareaClasses}
          {...props}
        />
        {error && (
          <p className="mt-1 text-sm text-red-600 dark:text-red-400" role="alert">
            {error}
          </p>
        )}
        {helperText && !error && (
          <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
            {helperText}
          </p>
        )}
      </div>
    );
  }
);

FormTextarea.displayName = 'FormTextarea';
