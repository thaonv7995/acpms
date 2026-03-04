import { ReactNode } from 'react';
import {
  PanelGroup,
  Panel,
  PanelResizeHandle,
} from 'react-resizable-panels';
import { motion, AnimatePresence } from 'framer-motion';
import { cn } from '@/lib/utils';

export type LayoutMode = 'preview' | 'diffs' | null;

interface RightWorkAreaProps {
  attempt: ReactNode;
  aux?: ReactNode;
  rightHeader?: ReactNode;
  mode: LayoutMode;
  auxSizes: [number, number];
  onAuxResize: (sizes: number[]) => void;
}

const MIN_PANEL_SIZE = 20;

/**
 * AuxRouter - Handles AnimatePresence transitions for preview/diffs
 */
function AuxRouter({ mode, aux }: { mode: LayoutMode; aux?: ReactNode }) {
  return (
    <AnimatePresence initial={false} mode="wait">
      {mode && aux && (
        <motion.div
          key={mode}
          initial={{ x: 168 }}
          animate={{ x: 0 }}
          exit={{ x: -96 }}
          transition={{ duration: 0.62, ease: [0.22, 1, 0.36, 1] }}
          className="h-full min-h-0 will-change-transform"
        >
          {aux}
        </motion.div>
      )}
    </AnimatePresence>
  );
}

/**
 * RightWorkArea - Contains header and Attempt/Aux content.
 * - When mode === null: Shows just Attempt panel (STATE 2)
 * - When mode !== null: Shows Attempt | Aux split (STATE 3)
 */
export function RightWorkArea({
  attempt,
  aux,
  rightHeader,
  mode,
  auxSizes,
  onAuxResize,
}: RightWorkAreaProps) {
  const showAux = mode !== null && aux;

  return (
    <div className="h-full min-h-0 flex flex-col bg-background">
      {/* Sticky header with mode toggle */}
      {rightHeader && (
        <div className="shrink-0 sticky top-0 z-20 bg-background border-b border-border">
          {rightHeader}
        </div>
      )}

      <div className="flex-1 min-h-0 overflow-hidden">
        {!showAux ? (
          // STATE 2: mode=null, show attempt only
          <div className="h-full min-h-0">{attempt}</div>
        ) : (
          // STATE 3: mode='preview'|'diffs', split Attempt | Aux
          <PanelGroup
            direction="horizontal"
            className="h-full min-h-0"
            onLayout={(layout) => {
              if (layout.length === 2) {
                onAuxResize([layout[0], layout[1]]);
              }
            }}
          >
            {/* Attempt Panel (34%) */}
            <Panel
              id="attempt"
              order={1}
              defaultSize={auxSizes[0]}
              minSize={MIN_PANEL_SIZE}
              collapsible
              collapsedSize={0}
              className="min-w-0 min-h-0 overflow-hidden"
              role="region"
              aria-label="Task Details"
            >
              {attempt}
            </Panel>

            <PanelResizeHandle
              id="handle-aa"
              className={cn(
                'relative z-30 bg-border cursor-col-resize group touch-none',
                'focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/60',
                'focus-visible:ring-offset-1 focus-visible:ring-offset-background',
                'transition-all w-1'
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

            {/* Aux Panel - Preview or Diffs (66%) */}
            <Panel
              id="aux"
              order={2}
              defaultSize={auxSizes[1]}
              minSize={MIN_PANEL_SIZE}
              collapsible={false}
              className="min-w-0 min-h-0 overflow-hidden"
              role="region"
              aria-label={mode === 'preview' ? 'Preview' : 'Diffs'}
            >
              <AuxRouter mode={mode} aux={aux} />
            </Panel>
          </PanelGroup>
        )}
      </div>
    </div>
  );
}
