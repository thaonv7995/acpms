/**
 * RightPanelContainer - Container that manages navigation between Agent Session and Diff Viewer
 *
 * Features:
 * - Switches between AgentSessionPanel and DiffViewer
 * - Back navigation from DiffViewer to AgentSession
 * - Maintains view stack for navigation history
 * - Passes through task and attempt context
 */

import { useState, useCallback, useEffect } from 'react';
import type { KanbanTask } from '../../types/project';
import { AgentSessionPanel } from './AgentSessionPanel';
import { DiffViewer } from '../diff-viewer';

type RightPanelView = 'agent-session' | 'diff-viewer';

interface RightPanelContainerProps {
  task: KanbanTask;
  taskId: string;
  attemptId?: string;
  projectId?: string;
  onClose: () => void;
  initialView?: RightPanelView;
}

export function RightPanelContainer({
  task,
  taskId,
  attemptId,
  projectId,
  onClose,
  initialView = 'agent-session',
}: RightPanelContainerProps) {
  const [currentView, setCurrentView] = useState<RightPanelView>(initialView);

  // Navigate to diff viewer when file change is clicked
  const handleViewDiff = useCallback((_attemptIdParam?: string) => {
    if (_attemptIdParam || attemptId) {
      setCurrentView('diff-viewer');
    }
  }, [attemptId]);

  // Navigate back to agent session
  const handleBackToSession = useCallback(() => {
    setCurrentView('agent-session');
  }, []);

  // Handle action complete (approve/reject/request changes)
  const handleActionComplete = useCallback(() => {
    // After any action, go back to agent session
    setCurrentView('agent-session');
  }, []);

  // Handle keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (currentView === 'diff-viewer') {
          handleBackToSession();
        } else {
          onClose();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [currentView, handleBackToSession, onClose]);

  if (currentView === 'diff-viewer' && attemptId) {
    return (
      <DiffViewer
        attemptId={attemptId}
        taskTitle={task.title}
        onBack={handleBackToSession}
        onClose={onClose}
        onActionComplete={handleActionComplete}
      />
    );
  }

  return (
    <AgentSessionPanel
      task={task}
      taskId={taskId}
      attemptId={attemptId}
      projectId={projectId}
      onClose={onClose}
      onViewDiff={handleViewDiff}
    />
  );
}
