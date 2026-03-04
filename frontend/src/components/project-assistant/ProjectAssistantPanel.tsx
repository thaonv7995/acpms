// PA-107 + PA-206 + PA-404: Project Assistant - compact chat form (Zalo/Messenger style)
import { useState, useRef, useEffect } from 'react';
import { ProjectAssistantChat } from './ProjectAssistantChat';
import { SessionMenu } from './SessionMenu';

interface AssistantMessage {
  id: string;
  session_id: string;
  role: string;
  content: string;
  metadata?: { tool_calls?: Array<{ id: string; name: string; args: Record<string, unknown> }> };
  created_at: string;
}

interface ProjectAssistantPanelProps {
  projectId: string;
  sessionId: string | null;
  sessionStatus?: string;
  messages: AssistantMessage[];
  error?: string | null;
  agentActive?: boolean;
  starting?: boolean;
  onStartAgent?: () => void;
  onSendMessage: (content: string) => Promise<boolean>;
  onRefreshMessages?: () => void;
  onLoadSession?: (sessionId: string) => void;
  onEndSession?: (sessionId: string) => void;
  onNewSession?: () => void;
  loading?: boolean;
  onClose: () => void;
  onRefreshProject?: () => void;
}

export function ProjectAssistantPanel({
  projectId,
  sessionId,
  sessionStatus = 'active',
  messages,
  error,
  agentActive = false,
  starting = false,
  onStartAgent,
  onSendMessage,
  onRefreshMessages,
  onLoadSession,
  onEndSession,
  onNewSession,
  loading = false,
  onClose,
}: ProjectAssistantPanelProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    if (menuOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [menuOpen]);

  return (
    <div className="fixed bottom-6 right-6 z-[9998] flex flex-col">
      {/* Compact chat form - Zalo/Messenger style */}
      <div
        className={`h-[520px] max-h-[calc(100vh-120px)] bg-card rounded-2xl border border-border shadow-xl flex flex-col overflow-hidden transition-all duration-200 ${
          expanded ? 'w-[560px]' : 'w-[380px]'
        }`}
        role="dialog"
        aria-label="Project Assistant"
        data-project-id={projectId}
      >
        {/* Header - compact */}
        <div className="flex items-center justify-between px-3 py-2.5 border-b border-border bg-card/95 shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            <div className="w-8 h-8 rounded-xl bg-primary/15 flex items-center justify-center shrink-0">
              <span className="material-symbols-outlined text-primary text-lg">smart_toy</span>
            </div>
            <span className="text-sm font-semibold text-card-foreground truncate">
              Project Assistant
            </span>
          </div>
          <div className="flex items-center gap-0.5 relative shrink-0" ref={menuRef}>
            <button
              onClick={() => setExpanded((e) => !e)}
              className="p-1.5 rounded-lg text-muted-foreground hover:bg-muted hover:text-card-foreground transition-colors"
              aria-label={expanded ? 'Collapse' : 'Expand'}
              title={expanded ? 'Collapse' : 'Expand'}
            >
              <span className="material-symbols-outlined text-xl">
                {expanded ? 'close_fullscreen' : 'open_in_full'}
              </span>
            </button>
            <button
              onClick={() => setMenuOpen((o) => !o)}
              className="p-1.5 rounded-lg text-muted-foreground hover:bg-muted hover:text-card-foreground transition-colors"
              aria-label="Session menu"
              aria-expanded={menuOpen}
            >
              <span className="material-symbols-outlined text-xl">more_vert</span>
            </button>
            <SessionMenu
              projectId={projectId}
              currentSessionId={sessionId}
              onLoadSession={onLoadSession ?? (() => {})}
              onEndSession={onEndSession ?? (() => {})}
              onNewSession={onNewSession ?? (() => {})}
              onClose={onClose}
              isOpen={menuOpen}
              onToggle={() => setMenuOpen(false)}
            />
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg text-muted-foreground hover:bg-muted hover:text-card-foreground transition-colors"
              aria-label="Đóng"
            >
              <span className="material-symbols-outlined text-xl">close</span>
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
          {error && (
            <div className="mx-3 mt-2 p-2 rounded-lg bg-destructive/10 text-destructive text-xs">
              {error}
            </div>
          )}
          {sessionId ? (
            <ProjectAssistantChat
              projectId={projectId}
              sessionId={sessionId}
              messages={messages}
              agentActive={agentActive}
              starting={starting}
              onStartAgent={onStartAgent}
              onSendMessage={onSendMessage}
              onRefresh={onRefreshMessages ?? (() => {})}
              loading={loading}
              readOnly={sessionStatus === 'ended'}
            />
          ) : (
            <div className="flex-1 flex items-center justify-center p-4">
              <p className="text-muted-foreground text-sm">Creating session...</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
