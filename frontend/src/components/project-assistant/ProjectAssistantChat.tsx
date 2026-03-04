// PA-206 + PA-304: ProjectAssistantChat - compact chat form (Zalo/Messenger style)
import { useRef, useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

const ASSISTANT_MD_COMPONENTS = {
  p: ({ children }: { children?: React.ReactNode }) => (
    <p className="my-1 whitespace-pre-wrap break-words">{children}</p>
  ),
  pre: ({ children }: { children?: React.ReactNode }) => (
    <pre className="my-2 overflow-x-auto rounded-lg bg-muted/60 p-2.5 text-xs font-mono whitespace-pre border border-border/50">
      {children}
    </pre>
  ),
  code: ({ className, children }: { className?: string; children?: React.ReactNode }) =>
    className ? (
      <code className={className}>{children}</code>
    ) : (
      <code className="rounded bg-muted/60 px-1 py-0.5 text-xs font-mono">{children}</code>
    ),
};

const TYPEWRITER_MS = 12;
const TYPEWRITER_MAX_DURATION_MS = 1800;

function TypewriterMarkdown({ content, className }: { content: string; className?: string }) {
  const [visibleLength, setVisibleLength] = useState(0);
  const len = content.length;

  useEffect(() => {
    if (visibleLength >= len) return;
    const step = Math.max(1, Math.ceil(len / (TYPEWRITER_MAX_DURATION_MS / TYPEWRITER_MS)));
    const id = setTimeout(() => {
      setVisibleLength((v) => Math.min(v + step, len));
    }, TYPEWRITER_MS);
    return () => clearTimeout(id);
  }, [visibleLength, len]);

  const visible = content.slice(0, visibleLength);

  return (
    <div className={className}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={ASSISTANT_MD_COMPONENTS}>
        {visible}
      </ReactMarkdown>
      {visibleLength < len && (
        <span className="inline-block w-0.5 h-[1em] align-middle bg-current animate-pulse ml-0.5" />
      )}
    </div>
  );
}

function WaitingForAgentHint() {
  const [showHint, setShowHint] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setShowHint(true), 15000);
    return () => clearTimeout(t);
  }, []);
  return (
    <div className="flex flex-col items-center py-6 gap-2">
      <div className="w-5 h-5 border-2 border-primary border-t-transparent rounded-full animate-spin" />
      <p className="text-muted-foreground text-xs">Agent is starting...</p>
      {showHint && (
        <p className="text-muted-foreground/70 text-[11px] text-center max-w-[220px]">
          Try sending a message or Session → New if there is no response.
        </p>
      )}
    </div>
  );
}
import { ToolConfirmationCard } from './ToolConfirmationCard';
import {
  getAssistantAttachmentUploadUrl,
  type ToolCall,
  type AttachmentRef,
} from '@/api/projectAssistant';

interface AssistantMessage {
  id: string;
  session_id: string;
  role: string;
  content: string;
  metadata?: { tool_calls?: ToolCall[] };
  created_at: string;
}

/** Gộp các tin nhắn assistant liên tiếp thành một (backend stream mỗi chunk = 1 message) */
function mergeConsecutiveAssistantMessages(msgs: AssistantMessage[]): AssistantMessage[] {
  const out: AssistantMessage[] = [];
  let buf: AssistantMessage[] = [];
  for (const m of msgs) {
    if (m.role === 'assistant') {
      buf.push(m);
    } else {
      if (buf.length > 0) {
        out.push({
          ...buf[0],
          content: buf.map((x) => x.content).join(''),
          metadata: buf[buf.length - 1]?.metadata,
        });
        buf = [];
      }
      out.push(m);
    }
  }
  if (buf.length > 0) {
    out.push({
      ...buf[0],
      content: buf.map((x) => x.content).join(''),
      metadata: buf[buf.length - 1]?.metadata,
    });
  }
  return out;
}

/** Prefixes to strip only at the very start of the message (e.g. greeting) */
const INTERNAL_PREFIXES_START = [
  'Delivering brief greeting',
  'Preparing brief greeting',
  'Preparing initial codebase inspection',
];

/** Line prefixes/phrases that mark internal status – entire line is removed. */
const INTERNAL_LINE_PATTERNS: (string | RegExp)[] = [
  'Providing minimal user update before inspection',
  'Reviewing agent instructions',
  'Seeking render types details',
  'Seeking ',
  'Exploring external UI components',
  'Exploring ',
  'Inspecting UI components',
  'Inspecting ',
  'Considering container component',
  'Considering additional design section',
  'Considering ',
  'Assessing conversation list components',
  'Assessing ',
  'Preparing base chat styling',
  'Preparing ',
  'Summarizing current chat UI design',
  'Summarizing ',
  'Extending ',
  'Confirming ',
  'Delivering brief greeting',
  'Preparing brief greeting',
  'Preparing initial codebase inspection',
  /^\*{2,}\s*\S/,  // ****Something
];

function isInternalLine(line: string): boolean {
  const t = line.trim();
  if (!t) return true;
  for (const p of INTERNAL_LINE_PATTERNS) {
    if (typeof p === 'string') {
      if (t.startsWith(p)) return true;
    } else {
      if (p.test(t)) return true;
    }
  }
  return false;
}

function stripInternalPrefixes(text: string): string {
  let out = text;
  for (const p of INTERNAL_PREFIXES_START) {
    if (out.startsWith(p)) out = out.slice(p.length).trimStart();
  }
  const lines = out.split('\n');
  const kept = lines.filter((line) => !isInternalLine(line));
  return kept.join('\n').replace(/\n{3,}/g, '\n\n').trim();
}

/** Ẩn log spawn/status và internal status (Preparing...) - chỉ hiện user, assistant response thật, và system Error */
function shouldShowMessage(m: AssistantMessage): boolean {
  if (m.role === 'user') return true;
  if (m.role === 'system') {
    if (m.content.startsWith('Error:')) return true;
    const hasToolCalls = Boolean(m.metadata?.tool_calls?.length);
    return hasToolCalls;
  }
  if (m.role === 'assistant') {
    const cleaned = stripInternalPrefixes(m.content);
    const hasToolCalls = Boolean(m.metadata?.tool_calls?.length);
    return cleaned.length > 0 || hasToolCalls;
  }
  return m.role !== 'stderr';
}

interface ProjectAssistantChatProps {
  projectId: string;
  sessionId: string;
  messages: AssistantMessage[];
  agentActive?: boolean;
  starting?: boolean;
  onStartAgent?: () => void;
  onSendMessage: (content: string, attachments?: AttachmentRef[]) => Promise<boolean>;
  onRefresh: () => void;
  loading?: boolean;
  readOnly?: boolean;
}

export function ProjectAssistantChat({
  projectId,
  sessionId,
  messages,
  agentActive = false,
  starting = false,
  onStartAgent,
  onSendMessage,
  onRefresh,
  loading = false,
  readOnly = false,
}: ProjectAssistantChatProps) {
  const [input, setInput] = useState('');
  const [sending, setSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevMsgCountRef = useRef(0);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const count = messages.length;
    if (count > prevMsgCountRef.current) {
      prevMsgCountRef.current = count;
      el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
    } else {
      prevMsgCountRef.current = count;
      const isNearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
      if (isNearBottom) {
        el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' });
      }
    }
  }, [messages]);

  const [attachments, setAttachments] = useState<AttachmentRef[]>([]);
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [input]);

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files?.length || uploading || readOnly) return;
    setUploading(true);
    try {
      const MAX_FILES = 5;
      const MAX_SIZE = 1024 * 1024; // 1MB
      const allowedTypes = ['text/', 'application/json'];
      for (let i = 0; i < Math.min(files.length, MAX_FILES - attachments.length); i++) {
        const file = files[i];
        if (file.size > MAX_SIZE) continue;
        const allowed = allowedTypes.some((t) => file.type.startsWith(t) || file.type === t);
        if (!allowed && file.type) continue;
        const { upload_url, key } = await getAssistantAttachmentUploadUrl(
          projectId,
          file.name,
          file.type || 'text/plain'
        );
        const res = await fetch(upload_url, {
          method: 'PUT',
          headers: { 'Content-Type': file.type || 'text/plain' },
          body: file,
        });
        if (res.ok) {
          setAttachments((prev) => [...prev, { key, filename: file.name }]);
        }
      }
    } finally {
      setUploading(false);
      e.target.value = '';
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = input.trim();
    if (!trimmed || sending) return;
    setSending(true);
    try {
      const ok = await onSendMessage(trimmed, attachments.length > 0 ? attachments : undefined);
      if (ok) {
        setInput('');
        setAttachments([]);
      }
    } finally {
      setSending(false);
    }
  };

  const visibleMessages = messages.filter(shouldShowMessage);
  const mergedMessages = mergeConsecutiveAssistantMessages(visibleMessages);
  const hasAssistantReply = mergedMessages.some((m) => m.role === 'assistant');
  const lastMessageIsUser = mergedMessages.length > 0 && mergedMessages[mergedMessages.length - 1].role === 'user';
  const showTyping = agentActive && (loading || lastMessageIsUser);
  const showStartPrompt = !readOnly && !agentActive && !hasAssistantReply;

  return (
    <div className="flex flex-col h-full bg-background/30">
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto px-3 py-3 space-y-2.5 min-h-0 scroll-smooth"
      >
        {showStartPrompt && (
          <div className="flex flex-col items-center justify-center py-12 gap-4">
            {starting ? (
              <div className="flex flex-col items-center gap-2">
                <div className="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin" />
                <p className="text-muted-foreground text-xs">Agent is starting...</p>
              </div>
            ) : (
              <>
                <div className="w-12 h-12 rounded-2xl bg-primary/15 flex items-center justify-center">
                  <span className="material-symbols-outlined text-2xl text-primary">smart_toy</span>
                </div>
                <p className="text-muted-foreground text-xs text-center max-w-[200px]">
                  Click Start to begin
                </p>
                <button
                  onClick={onStartAgent}
                  disabled={starting}
                  className="px-6 py-2.5 bg-primary hover:bg-primary/90 disabled:opacity-50 text-primary-foreground rounded-xl text-sm font-medium transition-all hover:scale-[1.02] active:scale-[0.98]"
                >
                  Start
                </button>
              </>
            )}
          </div>
        )}
        {agentActive && mergedMessages.length === 0 && (
          <WaitingForAgentHint />
        )}
        {mergedMessages.map((m, idx) => {
          const isLastAssistant = m.role === 'assistant' && idx === mergedMessages.length - 1;
          return (
          <div
            key={m.id}
            className={`flex ${m.role === 'user' ? 'justify-end' : 'justify-start'} animate-in fade-in duration-150`}
          >
            <div
              className={`max-w-[88%] rounded-2xl px-3 py-2 text-[13px] shadow-sm ${
                m.role === 'user'
                  ? 'bg-primary text-primary-foreground rounded-br-md'
                  : m.role === 'system' && m.content.startsWith('Error:')
                    ? 'bg-destructive/15 text-destructive border border-destructive/30'
                    : 'bg-muted/80 text-card-foreground rounded-bl-md'
              }`}
            >
              {m.content && (
                m.role === 'assistant' ? (
                  <div className="prose prose-sm dark:prose-invert max-w-none max-h-[200px] overflow-y-auto overflow-x-hidden [&_*]:text-inherit [&_ul]:my-1 [&_ol]:my-1">
                    {isLastAssistant ? (
                      <TypewriterMarkdown content={stripInternalPrefixes(m.content)} />
                    ) : (
                      <ReactMarkdown remarkPlugins={[remarkGfm]} components={ASSISTANT_MD_COMPONENTS}>
                        {stripInternalPrefixes(m.content)}
                      </ReactMarkdown>
                    )}
                  </div>
                ) : (
                  <p className="whitespace-pre-wrap break-words leading-relaxed max-h-[200px] overflow-y-auto">{m.content}</p>
                )
              )}
              {m.metadata?.tool_calls?.map((tc) => (
                <ToolConfirmationCard
                  key={tc.id}
                  projectId={projectId}
                  sessionId={sessionId}
                  toolCall={tc}
                  onConfirmed={onRefresh}
                />
              ))}
            </div>
          </div>
          );
        })}
        {showTyping && (
          <div className="flex justify-start animate-in fade-in duration-200">
            <div className="flex items-center gap-1.5 bg-muted/80 rounded-2xl rounded-bl-md px-3 py-2.5 shadow-sm">
              <span className="w-2 h-2 rounded-full bg-muted-foreground/70 animate-bounce" style={{ animationDelay: '0ms' }} />
              <span className="w-2 h-2 rounded-full bg-muted-foreground/70 animate-bounce" style={{ animationDelay: '150ms' }} />
              <span className="w-2 h-2 rounded-full bg-muted-foreground/70 animate-bounce" style={{ animationDelay: '300ms' }} />
            </div>
          </div>
        )}
      </div>

      {!readOnly && (agentActive || hasAssistantReply) && (
        <form onSubmit={handleSubmit} className="p-2.5 border-t border-border bg-card/60 shrink-0">
          {attachments.length > 0 && (
            <div className="mb-1.5 flex flex-wrap gap-1.5">
              {attachments.map((a, i) => (
                <span
                  key={a.key}
                  className="inline-flex items-center gap-1 rounded-md bg-muted px-2 py-0.5 text-[11px]"
                >
                  {a.filename ?? a.key}
                  <button
                    type="button"
                    onClick={() =>
                      setAttachments((prev) => prev.filter((_, j) => j !== i))
                    }
                    className="text-muted-foreground hover:text-destructive transition-colors"
                  >
                    <span className="material-symbols-outlined text-xs">close</span>
                  </button>
                </span>
              ))}
            </div>
          )}
          <div className="flex gap-1.5 items-end">
            <input
              ref={fileInputRef}
              type="file"
              accept="text/*,application/json"
              onChange={handleFileSelect}
              className="hidden"
            />
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              disabled={uploading || attachments.length >= 5}
              className="p-2 rounded-lg text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-50 transition-colors shrink-0"
              title="Attach file"
            >
              <span className="material-symbols-outlined text-lg">attach_file</span>
            </button>
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleSubmit(e);
                }
              }}
              placeholder="Type a message..."
              className="flex-1 min-h-[38px] max-h-[200px] px-3 py-2 rounded-xl border border-border bg-background text-foreground text-[13px] resize-none overflow-y-auto focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary placeholder:text-muted-foreground"
              rows={1}
              disabled={sending}
            />
            <button
              type="submit"
              disabled={!input.trim() || sending}
              className="p-2 rounded-lg bg-primary hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed text-primary-foreground transition-all shrink-0 hover:scale-105 active:scale-95"
              title="Send"
            >
              <span className="material-symbols-outlined text-lg">send</span>
            </button>
          </div>
        </form>
      )}
    </div>
  );
}
