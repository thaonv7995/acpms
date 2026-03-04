// PA-404: SessionMenu - Session history, End, New
import { useState, useEffect } from 'react';
import { listSessions, type AssistantSession } from '@/api/projectAssistant';

interface SessionMenuProps {
  projectId: string;
  currentSessionId: string | null;
  onLoadSession: (sessionId: string) => void;
  onEndSession: (sessionId: string) => void;
  onNewSession: () => void;
  onClose: () => void;
  isOpen: boolean;
  onToggle: () => void;
}

export function SessionMenu({
  projectId,
  currentSessionId,
  onLoadSession,
  onEndSession,
  onNewSession,
  onClose,
  isOpen,
  onToggle,
}: SessionMenuProps) {
  const [sessions, setSessions] = useState<AssistantSession[]>([]);
  const [loadingSessions, setLoadingSessions] = useState(false);
  const [showHistory, setShowHistory] = useState(false);

  useEffect(() => {
    if (isOpen && showHistory && projectId) {
      setLoadingSessions(true);
      listSessions(projectId)
        .then(setSessions)
        .catch(() => setSessions([]))
        .finally(() => setLoadingSessions(false));
    }
  }, [isOpen, showHistory, projectId]);

  const handleSessionHistoryClick = () => {
    setShowHistory((prev) => !prev);
  };

  const handleSelectSession = (sessionId: string) => {
    onLoadSession(sessionId);
    setShowHistory(false);
    onToggle();
  };

  const handleEnd = () => {
    if (currentSessionId) {
      onEndSession(currentSessionId);
      onToggle();
      onClose();
    }
  };

  const handleNew = () => {
    onNewSession();
    setShowHistory(false);
    onToggle();
  };

  if (!isOpen) return null;

  return (
    <div
      className="absolute right-0 top-12 mt-1 py-1 bg-card border border-border rounded-lg shadow-lg min-w-[200px] z-10"
      role="menu"
    >
      <button
        className="w-full px-4 py-2 text-left text-sm text-muted-foreground hover:bg-muted hover:text-card-foreground flex items-center justify-between"
        onClick={handleSessionHistoryClick}
        role="menuitem"
      >
        Session history
        <span className="material-symbols-outlined text-base">
          {showHistory ? 'expand_less' : 'expand_more'}
        </span>
      </button>
      {showHistory && (
        <div className="max-h-48 overflow-y-auto border-t border-border">
          {loadingSessions ? (
            <div className="px-4 py-2 text-sm text-muted-foreground">Loading...</div>
          ) : sessions.length === 0 ? (
            <div className="px-4 py-2 text-sm text-muted-foreground">No sessions yet</div>
          ) : (
            sessions.map((s) => (
              <button
                key={s.id}
                className={`w-full px-4 py-2 text-left text-sm hover:bg-muted ${
                  s.id === currentSessionId ? 'bg-muted/50 font-medium' : 'text-card-foreground'
                }`}
                onClick={() => handleSelectSession(s.id)}
                role="menuitem"
              >
                <span className="truncate block">
                  {s.status === 'active' ? '● ' : ''}
                  {new Date(s.created_at).toLocaleString()}
                </span>
              </button>
            ))
          )}
        </div>
      )}
      <button
        className="w-full px-4 py-2 text-left text-sm text-muted-foreground hover:bg-muted hover:text-card-foreground border-t border-border"
        onClick={handleEnd}
        disabled={!currentSessionId}
        role="menuitem"
      >
        End session
      </button>
      <button
        className="w-full px-4 py-2 text-left text-sm text-muted-foreground hover:bg-muted hover:text-card-foreground"
        onClick={handleNew}
        role="menuitem"
      >
        New session
      </button>
    </div>
  );
}
