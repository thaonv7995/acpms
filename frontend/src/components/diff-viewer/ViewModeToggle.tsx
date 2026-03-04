/**
 * ViewModeToggle - Toggle between side-by-side and unified diff views
 */

import { memo } from 'react';
import { clsx } from 'clsx';
import type { ViewMode } from './types';

interface ViewModeToggleProps {
  mode: ViewMode;
  onChange: (mode: ViewMode) => void;
  disabled?: boolean;
}

export const ViewModeToggle = memo(function ViewModeToggle({
  mode,
  onChange,
  disabled = false,
}: ViewModeToggleProps) {
  return (
    <div className="inline-flex items-center gap-1 p-0.5 rounded-sm border border-border bg-background">
      <button
        onClick={() => onChange('side-by-side')}
        disabled={disabled}
        className={clsx(
          'inline-flex items-center gap-1.5 h-6 px-2 rounded-sm text-xs font-medium transition-colors',
          mode === 'side-by-side'
            ? 'bg-muted text-foreground'
            : 'text-muted-foreground hover:text-foreground hover:bg-muted/40',
          disabled && 'opacity-50 cursor-not-allowed'
        )}
        title="Side by side view"
      >
        <span className="material-symbols-outlined text-[14px]">view_column_2</span>
        <span className="hidden sm:inline">Split</span>
      </button>
      <button
        onClick={() => onChange('unified')}
        disabled={disabled}
        className={clsx(
          'inline-flex items-center gap-1.5 h-6 px-2 rounded-sm text-xs font-medium transition-colors',
          mode === 'unified'
            ? 'bg-muted text-foreground'
            : 'text-muted-foreground hover:text-foreground hover:bg-muted/40',
          disabled && 'opacity-50 cursor-not-allowed'
        )}
        title="Unified view"
      >
        <span className="material-symbols-outlined text-[14px]">view_agenda</span>
        <span className="hidden sm:inline">Unified</span>
      </button>
    </div>
  );
});
