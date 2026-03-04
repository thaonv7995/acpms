import { PanelResizeHandle as ResizableHandle } from 'react-resizable-panels';
import { clsx } from 'clsx';

interface PanelResizeHandleProps {
  /** Direction of the resize handle */
  direction?: 'horizontal' | 'vertical';
  /** Whether the handle is currently being dragged */
  className?: string;
  /** Show grip indicator */
  showGrip?: boolean;
  /** ID for the handle element */
  id?: string;
}

/**
 * PanelResizeHandle - Custom styled resize handle for split panels.
 * Provides visual feedback during drag and hover states.
 * Matches dark theme of the application.
 *
 * @example
 * ```tsx
 * <PanelGroup direction="horizontal">
 *   <Panel>Left Content</Panel>
 *   <PanelResizeHandle direction="horizontal" showGrip />
 *   <Panel>Right Content</Panel>
 * </PanelGroup>
 * ```
 */
export function PanelResizeHandle({
  direction = 'horizontal',
  className,
  showGrip = true,
  id,
}: PanelResizeHandleProps) {
  const isHorizontal = direction === 'horizontal';

  return (
    <ResizableHandle
      id={id}
      className={clsx(
        'group relative flex items-center justify-center',
        'transition-colors duration-150',
        // Base styling
        isHorizontal ? 'w-1 cursor-col-resize' : 'h-1 cursor-row-resize',
        // Hover and active states
        'hover:bg-primary/30 data-[resize-handle-active]:bg-primary/50',
        // Background
        'bg-slate-200 dark:bg-slate-700',
        className
      )}
    >
      {/* Expanded hit area for easier grabbing */}
      <div
        className={clsx(
          'absolute',
          isHorizontal ? 'inset-y-0 -left-1 -right-1' : 'inset-x-0 -top-1 -bottom-1'
        )}
        aria-hidden="true"
      />

      {/* Grip indicator */}
      {showGrip && (
        <div
          className={clsx(
            'flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity',
            'data-[resize-handle-active]:opacity-100',
            isHorizontal ? 'flex-col' : 'flex-row'
          )}
        >
          {/* Three dot grip pattern */}
          <div className="w-1 h-1 rounded-full bg-slate-400 dark:bg-slate-500" />
          <div className="w-1 h-1 rounded-full bg-slate-400 dark:bg-slate-500" />
          <div className="w-1 h-1 rounded-full bg-slate-400 dark:bg-slate-500" />
        </div>
      )}
    </ResizableHandle>
  );
}
