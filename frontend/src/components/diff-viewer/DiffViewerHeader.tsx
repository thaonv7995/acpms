/**
 * DiffViewerHeader - Header for the DiffViewer panel
 */

import { memo, useEffect, useRef, useState } from 'react';
import { clsx } from 'clsx';

interface DiffViewerHeaderProps {
  attemptId?: string;
  taskTitle?: string;
  isLoading?: boolean;
  showExpandToggle?: boolean;
  areAllFilesExpanded?: boolean;
  onBack?: () => void;
  onClose?: () => void;
  onRefresh?: () => void;
  onOpenInNew?: () => void;
  onToggleExpandAll?: () => void;
}

export const DiffViewerHeader = memo(function DiffViewerHeader({
  attemptId,
  taskTitle,
  isLoading,
  showExpandToggle = false,
  areAllFilesExpanded = true,
  onBack,
  onClose,
  onRefresh,
  onOpenInNew,
  onToggleExpandAll,
}: DiffViewerHeaderProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onMouseDown = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', onMouseDown);
    return () => document.removeEventListener('mousedown', onMouseDown);
  }, [menuOpen]);

  const copyAttemptId = async () => {
    if (!attemptId) return;
    try {
      await navigator.clipboard.writeText(attemptId);
    } catch {
      // Ignore clipboard errors (e.g. blocked by browser policy)
    } finally {
      setMenuOpen(false);
    }
  };

  const handleOpenInNew = () => {
    onOpenInNew?.();
    setMenuOpen(false);
  };

  const handleRefresh = () => {
    onRefresh?.();
    setMenuOpen(false);
  };

  const handleClose = () => {
    onClose?.();
    setMenuOpen(false);
  };

  return (
    <div className="flex items-center justify-between gap-3 px-3 py-2 border-b border-dashed border-border bg-background">
      <div className="flex items-center gap-3">
        {/* Back button */}
        {onBack && (
          <button
            onClick={onBack}
            className="inline-flex items-center gap-1 h-7 px-2 border border-border rounded-sm text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
          >
            <span className="material-symbols-outlined text-[16px]">arrow_back</span>
            <span>Back</span>
          </button>
        )}

        {/* Title */}
        <h2 className="text-sm font-semibold text-foreground">Code Changes</h2>

        {taskTitle && (
          <span className="hidden sm:inline text-xs text-muted-foreground truncate max-w-[220px]">
            - {taskTitle}
          </span>
        )}
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-1">
        {onRefresh && (
          <button
            onClick={onRefresh}
            disabled={isLoading}
            className="h-7 w-7 inline-flex items-center justify-center rounded-sm text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors disabled:opacity-50"
            title="Refresh"
          >
            <span
              className={clsx('material-symbols-outlined text-[16px]', isLoading && 'animate-spin')}
            >
              refresh
            </span>
          </button>
        )}
        {showExpandToggle && (
          <button
            onClick={onToggleExpandAll}
            disabled={!onToggleExpandAll}
            className="h-7 w-7 inline-flex items-center justify-center rounded-sm text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors disabled:opacity-50"
            title={areAllFilesExpanded ? 'Collapse all files' : 'Expand all files'}
            aria-label={areAllFilesExpanded ? 'Collapse all files' : 'Expand all files'}
          >
            <span className="material-symbols-outlined text-[16px]">
              {areAllFilesExpanded ? 'unfold_less' : 'unfold_more'}
            </span>
          </button>
        )}
        <span className="h-4 w-px bg-border" />
        <button
          onClick={handleOpenInNew}
          disabled={!onOpenInNew}
          className="h-7 w-7 inline-flex items-center justify-center rounded-sm text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
          title="Open in new tab"
        >
          <span className="material-symbols-outlined text-[16px]">open_in_new</span>
        </button>
        <div className="relative" ref={menuRef}>
          <button
            onClick={() => setMenuOpen((value) => !value)}
            className="h-7 w-7 inline-flex items-center justify-center rounded-sm text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
            title="More actions"
          >
            <span className="material-symbols-outlined text-[16px]">more_vert</span>
          </button>
          {menuOpen && (
            <div className="absolute right-0 top-8 z-30 min-w-[180px] border border-border bg-background shadow-lg rounded-sm p-1">
              <button
                onClick={handleRefresh}
                disabled={!onRefresh}
                className="w-full text-left h-8 px-2 text-xs rounded-sm hover:bg-muted/50 text-foreground disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Refresh
              </button>
              <button
                onClick={handleOpenInNew}
                disabled={!onOpenInNew}
                className="w-full text-left h-8 px-2 text-xs rounded-sm hover:bg-muted/50 text-foreground disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Open in new tab
              </button>
              <button
                onClick={copyAttemptId}
                disabled={!attemptId}
                className="w-full text-left h-8 px-2 text-xs rounded-sm hover:bg-muted/50 text-foreground disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Copy attempt ID
              </button>
              {onClose && (
                <button
                  onClick={handleClose}
                  className="w-full text-left h-8 px-2 text-xs rounded-sm hover:bg-destructive/10 text-destructive"
                >
                  Close panel
                </button>
              )}
            </div>
          )}
        </div>
        {onClose && (
          <button
            onClick={handleClose}
            className="h-7 w-7 inline-flex items-center justify-center rounded-sm text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors"
            title="Close panel (Esc)"
          >
            <span className="material-symbols-outlined text-[16px]">close</span>
          </button>
        )}
      </div>
    </div>
  );
});
