import { ReactNode, useState } from 'react';
import { PanelGroup, Panel, PanelResizeHandle } from 'react-resizable-panels';
import { AnimatePresence, motion } from 'framer-motion';
import { cn } from '@/lib/utils';
import { RightWorkArea, type LayoutMode } from './RightWorkArea';

export type { LayoutMode };

interface TasksLayoutProps {
  kanban: ReactNode;
  attempt: ReactNode;
  aux?: ReactNode;  // Optional per spec
  isPanelOpen: boolean;
  mode: LayoutMode;
  isMobile?: boolean;
  rightHeader?: ReactNode;
  header?: ReactNode;  // Optional header per spec
}

type SplitSizes = [number, number];

const MIN_PANEL_SIZE = 20;
const DEFAULT_KANBAN_ATTEMPT: SplitSizes = [66, 34];
const DEFAULT_ATTEMPT_AUX: SplitSizes = [34, 66];

const STORAGE_KEYS = {
  KANBAN_ATTEMPT: 'tasksLayout.desktop.v2.kanbanAttempt',
  ATTEMPT_AUX: 'tasksLayout.desktop.v2.attemptAux',
} as const;

function loadSizes(key: string, fallback: SplitSizes): SplitSizes {
  try {
    const saved = localStorage.getItem(key);
    if (!saved) return fallback;
    const parsed = JSON.parse(saved);
    if (Array.isArray(parsed) && parsed.length === 2)
      return parsed as SplitSizes;
    return fallback;
  } catch {
    return fallback;
  }
}

function saveSizes(key: string, sizes: SplitSizes): void {
  try {
    localStorage.setItem(key, JSON.stringify(sizes));
  } catch {
    // Ignore errors
  }
}

/**
 * DesktopSimple - Conditionally renders layout based on mode.
 * When mode === null: Shows Kanban | Attempt
 * When mode !== null: Hides Kanban, shows only RightWorkArea with Attempt | Aux
 */
function DesktopSimple({
  kanban,
  attempt,
  aux,
  mode,
  rightHeader,
}: {
  kanban: ReactNode;
  attempt: ReactNode;
  aux?: ReactNode;
  mode: LayoutMode;
  rightHeader?: ReactNode;
}) {
  const [outerSizes] = useState<SplitSizes>(() =>
    loadSizes(STORAGE_KEYS.KANBAN_ATTEMPT, DEFAULT_KANBAN_ATTEMPT)
  );
  const [auxSizes, setAuxSizes] = useState<SplitSizes>(() =>
    loadSizes(STORAGE_KEYS.ATTEMPT_AUX, DEFAULT_ATTEMPT_AUX)
  );
  const [isKanbanCollapsed, setIsKanbanCollapsed] = useState(false);

  const handleAuxResize = (sizes: number[]) => {
    if (sizes.length === 2) {
      const newSizes: SplitSizes = [sizes[0], sizes[1]];
      setAuxSizes(newSizes);
      saveSizes(STORAGE_KEYS.ATTEMPT_AUX, newSizes);
    }
  };

  if (mode !== null) {
    return (
      <motion.div
        className="h-full min-h-0"
        initial={{ x: 96 }}
        animate={{ x: 0 }}
        transition={{ duration: 0.48, ease: [0.22, 1, 0.36, 1] }}
      >
        <RightWorkArea
          attempt={attempt}
          aux={aux}
          mode={mode}
          rightHeader={rightHeader}
          auxSizes={auxSizes}
          onAuxResize={handleAuxResize}
        />
      </motion.div>
    );
  }

  return (
    <PanelGroup
      direction="horizontal"
      className="h-full min-h-0"
      onLayout={(layout) => {
        if (layout.length === 2) {
          saveSizes(STORAGE_KEYS.KANBAN_ATTEMPT, [layout[0], layout[1]]);
        }
      }}
    >
      <Panel
        id="kanban"
        order={1}
        defaultSize={outerSizes[0]}
        minSize={MIN_PANEL_SIZE}
        collapsible
        collapsedSize={0}
        onCollapse={() => setIsKanbanCollapsed(true)}
        onExpand={() => setIsKanbanCollapsed(false)}
        className="h-full min-w-0 min-h-0 overflow-hidden"
        role="region"
        aria-label="Kanban board"
      >
        {kanban}
      </Panel>

      <PanelResizeHandle
        id="handle-kr"
        className={cn(
          'relative z-30 bg-border cursor-col-resize group touch-none h-full',
          'focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/60',
          'focus-visible:ring-offset-1 focus-visible:ring-offset-background',
          'transition-all',
          'border-l border-r border-border',
          isKanbanCollapsed ? 'w-6' : 'w-1'
        )}
        aria-label="Resize panels"
        role="separator"
        aria-orientation="vertical"
      >
        <div className="pointer-events-none absolute inset-y-0 left-1/2 -translate-x-1/2 w-px bg-border" />
        <div className="pointer-events-none absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 flex flex-col items-center gap-1 bg-muted/90 border border-border rounded-full px-1.5 py-3 opacity-70 group-hover:opacity-100 group-focus:opacity-100 transition-opacity shadow-sm">
          <span className="w-1 h-1 rounded-full bg-muted-foreground" />
          <span className="w-1 h-1 rounded-full bg-muted-foreground" />
          <span className="w-1 h-1 rounded-full bg-muted-foreground" />
        </div>
      </PanelResizeHandle>

      <Panel
        id="right"
        order={2}
        defaultSize={outerSizes[1]}
        minSize={MIN_PANEL_SIZE}
        collapsible={false}
        className="h-full min-w-0 min-h-0 overflow-hidden"
      >
        <motion.div
          className="h-full min-h-0"
          initial={{ x: 96 }}
          animate={{ x: 0 }}
          transition={{ duration: 0.48, ease: [0.22, 1, 0.36, 1] }}
        >
          <RightWorkArea
            attempt={attempt}
            aux={aux}
            mode={mode}
            rightHeader={rightHeader}
            auxSizes={auxSizes}
            onAuxResize={handleAuxResize}
          />
        </motion.div>
      </Panel>
    </PanelGroup>
  );
}

export function TasksLayout({
  kanban,
  attempt,
  aux,
  isPanelOpen,
  mode,
  isMobile = false,
  rightHeader,
  header,
}: TasksLayoutProps) {
  if (isMobile) {
    // Mobile layout: show only one panel at a time
    const showAux = isPanelOpen && mode !== null && aux;

    return (
      <div className="h-full min-h-0 flex flex-col bg-background">
        {/* Optional header (top-level) */}
        {header}

        {/* Panel header (when panel is open) */}
        {isPanelOpen && rightHeader && (
          <div className="shrink-0 sticky top-0 z-20 bg-background border-b border-border">
            {rightHeader}
          </div>
        )}

        <div className="flex-1 min-h-0">
          {!isPanelOpen ? (
            kanban
          ) : showAux ? (
            <AnimatePresence mode="wait">
              <motion.div
                key={mode}
                initial={{ x: 80 }}
                animate={{ x: 0 }}
                exit={{ x: -56 }}
                transition={{ duration: 0.42, ease: [0.22, 1, 0.36, 1] }}
                className="h-full"
              >
                {aux}
              </motion.div>
            </AnimatePresence>
          ) : (
            attempt
          )}
        </div>
      </div>
    );
  }

  // Desktop layout
  let desktopNode: ReactNode;

  if (!isPanelOpen) {
    // STATE 1: Just Kanban (panel closed)
    desktopNode = (
      <div
        className="h-full min-h-0 min-w-0 overflow-hidden"
        role="region"
        aria-label="Kanban board"
      >
        {kanban}
      </div>
    );
  } else {
    // STATE 2 & 3: Panel open (Kanban | Attempt or RightWorkArea with Attempt | Aux)
    desktopNode = (
      <DesktopSimple
        kanban={kanban}
        attempt={attempt}
        aux={aux}
        mode={mode}
        rightHeader={rightHeader}
      />
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Optional header */}
      {header}

      {/* Main content with animation */}
      <div className="flex-1 min-h-0">
        {desktopNode}
      </div>
    </div>
  );
}
