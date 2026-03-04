import * as React from 'react';
import { cn } from '@/lib/utils';

export interface AutoExpandingTextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  minHeight?: number;
  maxHeight?: number;
}

const AutoExpandingTextarea = React.forwardRef<
  HTMLTextAreaElement,
  AutoExpandingTextareaProps
>(({ className, minHeight = 40, maxHeight = 200, onChange, ...props }, ref) => {
  const textareaRef = React.useRef<HTMLTextAreaElement | null>(null);

  const adjustHeight = React.useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;

    // Reset height to auto to get correct scrollHeight
    textarea.style.height = 'auto';

    // Calculate new height within min/max bounds
    const newHeight = Math.min(
      Math.max(textarea.scrollHeight, minHeight),
      maxHeight
    );

    textarea.style.height = `${newHeight}px`;
  }, [minHeight, maxHeight]);

  React.useEffect(() => {
    adjustHeight();
  }, [adjustHeight, props.value]);

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    adjustHeight();
    onChange?.(e);
  };

  const handleRef = (element: HTMLTextAreaElement | null) => {
    textareaRef.current = element;
    if (typeof ref === 'function') {
      ref(element);
    } else if (ref) {
      ref.current = element;
    }
  };

  return (
    <textarea
      ref={handleRef}
      className={cn(
        'flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 resize-none overflow-y-auto transition-[height]',
        className
      )}
      style={{
        minHeight: `${minHeight}px`,
        maxHeight: `${maxHeight}px`,
      }}
      onChange={handleChange}
      {...props}
    />
  );
});

AutoExpandingTextarea.displayName = 'AutoExpandingTextarea';

export { AutoExpandingTextarea };
