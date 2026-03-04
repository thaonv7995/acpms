import React from 'react';
import { Panel, PanelGroup } from 'react-resizable-panels';
import { clsx } from 'clsx';
import { PanelResizeHandle } from './PanelResizeHandle';

interface SplitPanelLayoutProps {
  /** Left/top panel content (primary) */
  primaryContent: React.ReactNode;
  /** Right/bottom panel content (secondary) - if null, primary takes full width */
  secondaryContent?: React.ReactNode | null;
  /** Split direction */
  direction?: 'horizontal' | 'vertical';
  /** Default size of primary panel (percentage 0-100) */
  primaryDefaultSize?: number;
  /** Minimum size of primary panel (percentage 0-100) */
  primaryMinSize?: number;
  /** Maximum size of primary panel (percentage 0-100) */
  primaryMaxSize?: number;
  /** Minimum size of secondary panel (percentage 0-100) */
  secondaryMinSize?: number;
  /** Called when panel sizes change */
  onLayout?: (sizes: number[]) => void;
  /** Whether to show the resize handle grip indicator */
  showResizeGrip?: boolean;
  /** Custom class for the container */
  className?: string;
  /** Custom class for primary panel */
  primaryClassName?: string;
  /** Custom class for secondary panel */
  secondaryClassName?: string;
  /** ID for persisting panel sizes to localStorage */
  autoSaveId?: string;
}

/**
 * SplitPanelLayout - Main wrapper for react-resizable-panels.
 * Used for Kanban page split view with Agent Session panel.
 *
 * Features:
 * - Horizontal split (left: content, right: panel)
 * - Configurable default sizes
 * - Min/max size constraints
 * - Custom styled resize handle (dark theme)
 * - Auto-collapse when secondary content is null
 * - Optional localStorage persistence
 *
 * @example
 * ```tsx
 * // Basic horizontal split
 * <SplitPanelLayout
 *   primaryContent={<KanbanBoard />}
 *   secondaryContent={activeTask && <AgentSessionPanel task={activeTask} />}
 *   primaryDefaultSize={50}
 *   primaryMinSize={30}
 *   secondaryMinSize={40}
 * />
 *
 * // With persistence
 * <SplitPanelLayout
 *   primaryContent={<KanbanBoard />}
 *   secondaryContent={<AgentPanel />}
 *   autoSaveId="kanban-split"
 * />
 * ```
 */
export function SplitPanelLayout({
  primaryContent,
  secondaryContent,
  direction = 'horizontal',
  primaryDefaultSize = 50,
  primaryMinSize = 30,
  primaryMaxSize = 100,
  secondaryMinSize = 30,
  onLayout,
  showResizeGrip = true,
  className,
  primaryClassName,
  secondaryClassName,
  autoSaveId,
}: SplitPanelLayoutProps) {
  // When no secondary content, primary takes full width
  const hasSecondary = secondaryContent != null;
  const effectiveDefaultSize = hasSecondary ? primaryDefaultSize : 100;

  return (
    <PanelGroup
      direction={direction}
      onLayout={onLayout}
      autoSaveId={autoSaveId}
      className={clsx('h-full w-full', className)}
    >
      {/* Primary Panel (Kanban Board) */}
      <Panel
        id="primary-panel"
        defaultSize={effectiveDefaultSize}
        minSize={hasSecondary ? primaryMinSize : 100}
        maxSize={hasSecondary ? primaryMaxSize : 100}
        className={clsx(
          'flex flex-col overflow-hidden',
          'bg-slate-50 dark:bg-[#0d1117]',
          primaryClassName
        )}
      >
        {primaryContent}
      </Panel>

      {/* Resize Handle + Secondary Panel (Agent Session) */}
      {hasSecondary && (
        <>
          <PanelResizeHandle direction={direction} showGrip={showResizeGrip} />

          <Panel
            id="secondary-panel"
            defaultSize={100 - primaryDefaultSize}
            minSize={secondaryMinSize}
            className={clsx(
              'flex flex-col overflow-hidden',
              'bg-white dark:bg-slate-900',
              'border-l border-slate-200 dark:border-slate-700',
              secondaryClassName
            )}
          >
            {secondaryContent}
          </Panel>
        </>
      )}
    </PanelGroup>
  );
}

/**
 * Type exports for external use
 */
export type { SplitPanelLayoutProps };

/**
 * Re-export individual panel components for advanced use cases
 */
export { Panel, PanelGroup } from 'react-resizable-panels';
