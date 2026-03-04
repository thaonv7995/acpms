/**
 * Split Panel Components
 *
 * Resizable split panel layout for Kanban page with Agent Session.
 * Built on top of react-resizable-panels.
 *
 * @example
 * ```tsx
 * import { SplitPanelLayout, PanelResizeHandle } from '@/components/ui/split-panel';
 *
 * <SplitPanelLayout
 *   primaryContent={<KanbanBoard />}
 *   secondaryContent={activeTask && <AgentSessionPanel task={activeTask} />}
 *   primaryDefaultSize={50}
 *   primaryMinSize={30}
 *   secondaryMinSize={40}
 * />
 * ```
 */

export { SplitPanelLayout } from './SplitPanelLayout';
export type { SplitPanelLayoutProps } from './SplitPanelLayout';

export { PanelResizeHandle } from './PanelResizeHandle';

// Re-export underlying primitives for advanced use cases
export { Panel, PanelGroup } from 'react-resizable-panels';
