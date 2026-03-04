import { useEffect } from 'react';
import type { LayoutMode } from '../../components/layout/TasksLayout';

interface UseKeyboardShortcutsParams {
  isPanelOpen: boolean;
  mode: LayoutMode;
  onCreateTask: () => void;
  onClosePanel: () => void;
  onCycleMode: () => void;
}

/**
 * Custom hook for handling keyboard shortcuts in ProjectTasksPage
 */
export function useKeyboardShortcuts({
  isPanelOpen,
  mode,
  onCreateTask,
  onClosePanel,
  onCycleMode,
}: UseKeyboardShortcutsParams) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Cmd/Ctrl + K - Create task
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        onCreateTask();
      }

      // Escape - Close panel
      if (e.key === 'Escape' && isPanelOpen) {
        e.preventDefault();
        onClosePanel();
      }

      // Cmd/Ctrl + Enter - Cycle view mode
      if ((e.metaKey || e.ctrlKey) && e.key === 'Enter' && isPanelOpen) {
        e.preventDefault();
        onCycleMode();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isPanelOpen, mode, onCreateTask, onClosePanel, onCycleMode]);
}
