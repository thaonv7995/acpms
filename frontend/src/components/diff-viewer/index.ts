/**
 * Diff Viewer Components
 *
 * Export all diff viewer components for use in the application.
 */

// Main container
export { DiffViewer } from './DiffViewer';
export { DiffViewerHeader } from './DiffViewerHeader';

// Summary and info cards
export { FileSummaryCard } from './FileSummaryCard';
export { BranchInfoCard } from './BranchInfoCard';
export { GitActions } from './GitActions';

// View components
export { DiffContentArea } from './DiffContentArea';
export { SideBySideView } from './SideBySideView';
export { UnifiedView } from './UnifiedView';
export { ViewModeToggle } from './ViewModeToggle';

// Line components
export { DiffLine, UnifiedDiffLine } from './DiffLine';

// Hooks
export { useDiff, invalidateDiffCache } from './useDiff';
export type { UseDiffReturn, UseDiffOptions } from './useDiff';

// Types
export type {
  DiffFile,
  DiffLine as DiffLineType,
  DiffHunk,
  DiffFileStatus,
  DiffLineType as LineType,
  ViewMode,
  BranchInfo,
  DiffSummary,
  AvailableActions,
  DiffResponse,
  DiffUpdateEvent,
  DiffCompleteEvent,
} from './types';

export { STATUS_CONFIG, LANG_MAP, getLanguageFromPath, parseFilePath } from './types';
