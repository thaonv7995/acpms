import { useMemo, useState, type ComponentType, type MouseEvent, type ReactNode } from 'react';
import type {
  TimelineEntry,
  ToolCallEntry,
  OperationGroup,
  SubagentEntry,
} from '@/types/timeline-log';
import {
  AlertTriangle,
  Bot,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Edit3,
  Eye,
  Globe,
  Hammer,
  LoaderCircle,
  ListChecks,
  Pencil,
  RotateCcw,
  Search,
  Terminal,
  User,
  XCircle,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { parseTodoItems } from './todo-utils';
import { useApproval } from '@/hooks/useApproval';
import { formatShellCommandForDisplay } from '@/lib/commandDisplay';
import { humanizeLogText } from '@/lib/logPathDisplay';
import { ChatEntryContainer } from './ChatEntryContainer';
import { ChatToolSummary } from './ChatToolSummary';
import { ChatTodoList } from './ChatTodoList';
import { ChatCollapsedThinking } from './ChatCollapsedThinking';
import { ChatErrorMessage } from './ChatErrorMessage';
import { ChatFileToolRow } from './ChatFileToolRow';
import { timelineT } from './timeline-i18n';

interface TimelineEntryRendererProps {
  entry: TimelineEntry;
  onViewDiff?: (diffId: string, filePath?: string) => void;
  hideAvatarAndBorder?: boolean;
  isStreaming?: boolean;
  onEditUserMessage?: (entryId: string, content: string) => void;
  onResetUserMessage?: (entryId: string) => void;
  /** When true, user message shows "Queued" badge - agent will process when current task completes */
  showQueuedBadge?: boolean;
}

type LogTone = 'assistant' | 'tool' | 'error' | 'thinking' | 'user' | 'file' | 'subagent';

type ToolActionPayload = ToolCallEntry['actionType'] & {
  command?: string;
  query?: string;
  url?: string;
  todos?: unknown;
  arguments?: unknown;
  result?: unknown;
  changes?: unknown;
};



const SENSITIVE_PATTERNS: Array<{ regex: RegExp; replacement: string }> = [
  {
    regex: /(authorization:\s*bearer\s+)[^\s"']+/gi,
    replacement: '$1[REDACTED]',
  },
  {
    regex: /([A-Z0-9_]*(?:TOKEN|SECRET|PASSWORD|API_KEY)[A-Z0-9_]*\s*=\s*)[^\s"']+/gi,
    replacement: '$1[REDACTED]',
  },
  {
    regex: /("?(?:token|secret|password|api[_-]?key)"?\s*:\s*"?)([^"\s,}]+)/gi,
    replacement: '$1[REDACTED]',
  },
  {
    regex: /\b(gh[pousr]_[A-Za-z0-9_]{20,})\b/g,
    replacement: '[REDACTED]',
  },
  {
    regex: /\b(sk-[A-Za-z0-9]{20,})\b/g,
    replacement: '[REDACTED]',
  },
];

function redactSensitiveContent(text: string): string {
  const redacted = SENSITIVE_PATTERNS.reduce(
    (safeText, rule) => safeText.replace(rule.regex, rule.replacement),
    text
  );
  return humanizeLogText(redacted);
}

function formatDuration(ms?: number): string | null {
  if (!ms || Number.isNaN(ms)) return null;
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function compactInline(value: string): string {
  return value.replace(/\s+/g, ' ').trim();
}

function truncate(value: string, maxLength: number): string {
  if (value.length <= maxLength) return value;
  return `${value.slice(0, Math.max(0, maxLength - 1))}…`;
}

/** Format search/grep query for display: clean escapes, split by |, truncate. */
function formatSearchTarget(raw: string): string {
  let s = String(raw).trim();
  // Strip surrounding quotes (including \")
  s = s.replace(/^\\s*\\"?["']?|["']?\\"?\\s*$/g, '');
  // Unescape \. to .
  s = s.replace(/\\\./g, '.');
  s = s.trim();
  if (!s) return '';
  const terms = s.split(/\|/).map((t) => t.trim()).filter(Boolean);
  if (terms.length <= 1) return truncate(s, 60);
  const maxShow = 3;
  const shown = terms.slice(0, maxShow).join(', ');
  const extra = terms.length > maxShow ? ` (+${terms.length - maxShow})` : '';
  return truncate(shown + extra, 70);
}

function getToolActionLabel(action: string, toolName?: string): string {
  switch (action) {
    case 'todo_management':
      return 'Todo list updated';
    case 'command_run':
      return 'Ran command';
    case 'file_read':
      return 'Read file';
    case 'file_edit':
      return 'Edited file';
    case 'file_write':
      return 'Wrote file';
    case 'search':
      return 'Searched';
    case 'web_fetch':
      return 'Fetched URL';
    case 'task_create':
      return 'Created subtask';
    case 'plan_presentation':
      return 'Presented plan';
    default:
      return toolName || action || 'Tool action';
  }
}

function getToolIcon(action: string) {
  switch (action) {
    case 'todo_management':
      return ListChecks;
    case 'file_read':
      return Eye;
    case 'file_edit':
    case 'file_write':
      return Edit3;
    case 'search':
      return Search;
    case 'web_fetch':
      return Globe;
    case 'command_run':
      return Terminal;
    case 'task_create':
      return Bot;
    case 'plan_presentation':
      return ListChecks;
    default:
      return Hammer;
  }
}

type StatusBadge = {
  label: string;
  className: string;
  icon?: ComponentType<{ className?: string }>;
  iconClassName?: string;
} | null;

function getLinesFromAction(actionType: ToolActionPayload): number {
  const la = (actionType as { lines_added?: number }).lines_added;
  if (typeof la === 'number') return Math.max(0, la);
  const changes = actionType.changes;
  if (Array.isArray(changes)) {
    return changes.reduce(
      (sum, c) => sum + Math.max(0, (c as { lines_added?: number }).lines_added ?? 0),
      0
    );
  }
  return 0;
}

function getLinesRemovedFromAction(actionType: ToolActionPayload): number {
  const lr = (actionType as { lines_removed?: number }).lines_removed;
  if (typeof lr === 'number') return Math.max(0, lr);
  const changes = actionType.changes;
  if (Array.isArray(changes)) {
    return changes.reduce(
      (sum, c) => sum + Math.max(0, (c as { lines_removed?: number }).lines_removed ?? 0),
      0
    );
  }
  return 0;
}

function formatToolSummary(toolCall: ToolCallEntry): string {
  const action = toolCall.actionType?.action || 'tool';
  const actionPayload = toolCall.actionType as ToolActionPayload;
  const actionLabel = getToolActionLabel(action, toolCall.toolName);
  const target =
    actionPayload.file_path ||
    actionPayload.path ||
    actionPayload.target ||
    actionPayload.command ||
    actionPayload.query ||
    actionPayload.url;
  const todoItems =
    action === 'todo_management'
      ? parseTodoItems(actionPayload.todos ?? actionPayload.arguments, redactSensitiveContent)
      : [];

  let summary = actionLabel;
  if (action === 'todo_management' && todoItems.length > 0) {
    summary += ` (${todoItems.length})`;
  } else if (action === 'plan_presentation' && actionPayload.plan) {
    summary += `: ${truncate(compactInline(String(actionPayload.plan)), 120)}`;
  } else if (action === 'task_create' && actionPayload.description) {
    summary += `: ${truncate(compactInline(String(actionPayload.description)), 120)}`;
  } else if (action === 'search' && target) {
    const formatted = formatSearchTarget(String(target));
    summary += formatted ? `: ${formatted}` : '';
  } else if (target) {
    summary += `: ${truncate(compactInline(String(target)), 120)}`;
  }

  if (toolCall.status === 'success' && toolCall.duration) {
    const duration = formatDuration(toolCall.duration);
    if (duration) summary += ` · ${duration}`;
  }

  return redactSensitiveContent(summary);
}

function getStatusBadge(status?: ToolCallEntry['status']): StatusBadge {
  switch (status) {
    case 'running':
      return {
        label: 'Running',
        className: 'bg-primary/10 text-primary border-primary/30',
        icon: LoaderCircle,
        iconClassName: 'animate-spin',
      };
    case 'pending_approval':
      return {
        label: 'Needs approval',
        className: 'bg-amber-500/10 text-amber-500 border-amber-500/30',
      };
    case 'failed':
      return { label: 'Failed', className: 'bg-destructive/15 text-destructive border-destructive/30' };
    case 'denied':
      return { label: 'Denied', className: 'bg-destructive/15 text-destructive border-destructive/30' };
    case 'timed_out':
      return { label: 'Timed out', className: 'bg-destructive/15 text-destructive border-destructive/30' };
    case 'cancelled':
      return { label: 'Cancelled', className: 'bg-muted text-muted-foreground border-border' };
    default:
      return null;
  }
}

function formatGroupSummary(group: OperationGroup): string {
  const label =
    group.groupType === 'file_read'
      ? 'File Reads'
      : group.groupType === 'file_edit'
        ? 'File Edits'
        : 'Searches';

  const targets = group.operations
    .map((op) => {
      const at = op.actionType;
      const raw = at?.file_path || at?.path || at?.target || at?.query;
      return typeof raw === 'string' && raw.trim()
        ? (group.groupType === 'search' ? formatSearchTarget(raw) : redactSensitiveContent(raw))
        : null;
    })
    .filter(Boolean) as string[];

  const preview = targets.slice(0, 3).map(redactSensitiveContent).join(', ');
  const extraCount = targets.length > 3 ? ` +${targets.length - 3}` : '';
  const count = group.count || targets.length;

  if (!preview) {
    return `${count}x ${label}`;
  }
  return `${count}x ${label} · ${preview}${extraCount}`;
}

interface LogLineProps {
  icon?: ComponentType<{ className?: string }>;
  tone: LogTone;
  content: ReactNode;
  detail?: boolean;
  action?: ReactNode;
  onToggle?: () => void;
  expanded?: boolean;
  hideAvatarAndBorder?: boolean;
  rowClassName?: string;
}

function LogLine({
  icon: Icon,
  tone,
  content,
  detail = false,
  action,
  onToggle,
  expanded,
  hideAvatarAndBorder,
  rowClassName,
}: LogLineProps) {
  const isInteractive = Boolean(onToggle);
  const isUser = tone === 'user';

  const rowBg = isUser ? 'bg-muted/10' : 'bg-transparent';
  const detailTextClass = detail ? 'text-muted-foreground text-xs' : 'text-sm text-foreground';

  const lineBody = (
    <div className="flex gap-4 w-full">
      {/* Avatar */}
      {!detail && !hideAvatarAndBorder && (
        <div className="flex-shrink-0 mt-0.5">
          <div className={cn(
            "flex h-8 w-8 items-center justify-center rounded-md border",
            isUser ? "bg-secondary text-secondary-foreground border-border" : "bg-primary/10 text-primary border-primary/20",
            tone === 'error' && "bg-destructive/10 text-destructive border-destructive/20",
            tone === 'thinking' && "bg-muted shadow-sm text-muted-foreground border-border",
            tone === 'tool' && "bg-muted/50 text-foreground border-border",
            tone === 'file' && "bg-muted/50 text-foreground border-border"
          )}>
            {Icon ? <Icon className="h-4 w-4" /> : null}
          </div>
        </div>
      )}

      {/* Spacer for details align. If hideAvatarAndBorder, we don't need a spacer as the parent container has avatar */}
      {!hideAvatarAndBorder && detail && <div className="w-8 flex-shrink-0" />}

      {/* Content */}
      <div className="flex-1 min-w-0 flex flex-col gap-2 justify-center py-1">
        <div className={cn("leading-relaxed break-words whitespace-pre-wrap", detailTextClass, tone === 'thinking' && "italic")}>
          {content}
        </div>
        {action && <div className="mt-1">{action}</div>}
      </div>

      {/* Chevrons for toggle */}
      {isInteractive && (
        <div className="flex-shrink-0 mt-1.5 text-muted-foreground">
          {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
        </div>
      )}
    </div>
  );

  const wrapperClasses = cn(
    "w-full flex items-start py-2 transition-colors",
    !hideAvatarAndBorder && "px-4 py-3 border-b border-border/40",
    !hideAvatarAndBorder && rowBg,
    isInteractive && "cursor-pointer hover:bg-muted/30 focus:bg-muted/30 outline-none rounded-md",
    detail && "pt-0 pb-4 border-b-0",
    hideAvatarAndBorder && isInteractive && "px-2",
    rowClassName
  );

  if (isInteractive) {
    return (
      <div
        role="button"
        tabIndex={0}
        onClick={onToggle}
        onKeyDown={(event) => {
          if (!onToggle) return;
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            onToggle();
          }
        }}
        className={wrapperClasses}
      >
        {lineBody}
      </div>
    );
  }

  return (
    <div className={wrapperClasses}>
      {lineBody}
    </div>
  );
}

/**
 * Markdown content renderer. Vibe Kanban style: direct render, no animation.
 * Real-time feel comes from backend sending incremental updates.
 */
function MarkdownContent({ content }: { content: string }) {
  return (
    <div
      className={cn(
        'markdown-compact',
        'text-sm leading-6',
        '[&_p]:m-0 [&_p]:leading-6',
        '[&_ul]:m-0 [&_ul]:pl-3 [&_ul]:list-disc [&_ul]:list-inside',
        '[&_ol]:m-0 [&_ol]:pl-3 [&_ol]:list-decimal [&_ol]:list-inside',
        '[&_li]:m-0 [&_li]:p-0 [&_li]:leading-6',
        '[&_li>p]:inline [&_li>p]:m-0 [&_li>p]:leading-6',
        '[&_h1]:m-0 [&_h2]:m-0 [&_h3]:m-0',
        '[&_h1]:text-sm [&_h2]:text-sm [&_h3]:text-sm',
        '[&_h1]:font-semibold [&_h2]:font-semibold [&_h3]:font-semibold'
      )}
    >
      <ReactMarkdown
        children={content}
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noreferrer"
              className="text-[13px] font-medium text-sky-400 underline underline-offset-2 hover:text-sky-300 transition-colors"
            >
              {children}
            </a>
          ),
          pre: ({ children }) => {
            const codeChild =
              children && typeof children === 'object' && 'props' in children
                ? (children as { props?: { children?: unknown } }).props?.children
                : children;
            const codeText = Array.isArray(codeChild) ? codeChild.join('') : codeChild;
            const isSingleLine = typeof codeText === 'string' && !codeText.includes('\n');

            if (isSingleLine) {
              return <code className="text-foreground">{codeText}</code>;
            }

            return (
              <pre className="m-0 mt-2 mb-2 overflow-x-auto rounded-md border border-border/50 bg-muted/40 p-3 text-[13px] leading-5 text-foreground shadow-sm">
                {children}
              </pre>
            );
          },
          code: ({ children }) => <code className="text-foreground">{children}</code>,
        }}
      />
    </div>
  );
}

function compactMarkdown(source: string): string {
  if (!source) return source;
  let text = source.replace(/\r\n/g, '\n');
  text = text.replace(/[ \t]+\n/g, '\n');
  text = text.replace(/\n\s*\n+/g, '\n');
  text = text.replace(/^(#{1,6}[^\n]*)\n+/gm, '$1\n');
  return text;
}

function parseCommandOutput(result: unknown): string | null {
  if (!result || typeof result !== 'object') return null;
  const output = (result as { output?: unknown }).output;
  if (typeof output !== 'string') return null;
  const cleaned = output.trim();
  if (!cleaned) return null;
  return redactSensitiveContent(cleaned);
}

function ToolCallRows({
  toolCall,
  onViewDiff,
}: {
  toolCall: ToolCallEntry;
  onViewDiff?: (diffId: string, filePath?: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [approvalActionError, setApprovalActionError] = useState<string | null>(null);
  const actionType = toolCall.actionType as ToolActionPayload;
  const action = actionType.action || 'tool';
  const Icon = getToolIcon(action);
  const summary = formatToolSummary(toolCall);
  const hasApprovalId = Boolean(toolCall.approvalId);
  const isPendingApproval = toolCall.status === 'pending_approval' && hasApprovalId;
  const {
    approve,
    deny,
    isApproving,
    isDenying,
    error: approvalHookError,
  } = useApproval(toolCall.approvalId || '');
  const todoItems = useMemo(
    () => parseTodoItems(actionType.todos ?? actionType.arguments, redactSensitiveContent),
    [actionType.todos, actionType.arguments]
  );
  const commandOutput = useMemo(() => parseCommandOutput(actionType.result), [actionType.result]);
  const rawToolError = (toolCall as { error?: string }).error;
  const toolError =
    rawToolError ||
    (toolCall.status === 'denied' || toolCall.status === 'failed' || toolCall.status === 'timed_out'
      ? toolCall.statusReason
      : undefined);
  const hasCommandDetails = action === 'command_run' && Boolean(commandOutput);
  const hasDetails = hasCommandDetails || Boolean(toolError);
  const shouldRenderTodoCard = action === 'todo_management' && todoItems.length > 0;
  const statusBadge = getStatusBadge(toolCall.status);
  const approvalError = approvalActionError || approvalHookError;

  const handleApprove = async (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (!isPendingApproval || isApproving || isDenying) return;
    setApprovalActionError(null);
    try {
      await approve();
    } catch (error) {
      const message =
        error instanceof Error ? error.message : 'Failed to approve tool execution';
      setApprovalActionError(message);
    }
  };

  const handleDeny = async (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (!isPendingApproval || isApproving || isDenying) return;
    setApprovalActionError(null);
    try {
      await deny('Denied from timeline');
    } catch (error) {
      const message =
        error instanceof Error ? error.message : 'Failed to deny tool execution';
      setApprovalActionError(message);
    }
  };

  const toolHeaderRight =
    statusBadge || (toolCall.diffId && onViewDiff) ? (
      <div className="flex items-center gap-2">
        {statusBadge ? (
          <span
            className={cn(
              'inline-flex items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[10px] leading-none',
              statusBadge.className
            )}
          >
            {statusBadge.icon ? (
              <statusBadge.icon className={cn('h-3 w-3', statusBadge.iconClassName)} />
            ) : null}
            {statusBadge.label}
          </span>
        ) : null}
        {toolCall.diffId && onViewDiff ? (
          <button
            onClick={(event) => {
              event.stopPropagation();
              onViewDiff(
                toolCall.diffId!,
                toolCall.actionType?.file_path || toolCall.actionType?.path
              );
            }}
            className="text-[10px] text-primary hover:underline"
          >
            {timelineT.viewDiff}
          </button>
        ) : null}
      </div>
    ) : null;

  const approvalActions = isPendingApproval ? (
    <div className="flex items-center gap-1">
      <button
        type="button"
        onClick={handleApprove}
        disabled={isApproving || isDenying}
        className={cn(
          'inline-flex items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[10px] leading-none transition-colors',
          'border-emerald-500/40 text-emerald-400 hover:bg-emerald-500/15',
          (isApproving || isDenying) && 'opacity-60 cursor-not-allowed'
        )}
        title={timelineT.approve}
      >
        <CheckCircle2 className="h-3 w-3" />
        {timelineT.approve}
      </button>
      <button
        type="button"
        onClick={handleDeny}
        disabled={isApproving || isDenying}
        className={cn(
          'inline-flex items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[10px] leading-none transition-colors',
          'border-destructive/40 text-destructive hover:bg-destructive/15',
          (isApproving || isDenying) && 'opacity-60 cursor-not-allowed'
        )}
        title={timelineT.deny}
      >
        <XCircle className="h-3 w-3" />
        {timelineT.deny}
      </button>
    </div>
  ) : null;

  const actionArea = (toolHeaderRight || approvalActions) ? (
    <div className="flex items-center gap-2">
      {toolHeaderRight}
      {approvalActions}
    </div>
  ) : null;

  const toolStatus =
    toolCall.status && toolCall.status !== 'success'
      ? { status: toolCall.status }
      : undefined;

  const isPlanPresentation = action === 'plan_presentation';
  const planContent = actionType.plan;
  const planVariant =
    toolCall.status === 'denied' ? 'plan_denied' : 'plan';

  return (
    <>
      {isPlanPresentation && planContent ? (
        <ChatEntryContainer
          variant={planVariant}
          icon={ListChecks}
          title={summary}
          headerRight={toolHeaderRight}
          expanded={expanded}
          onToggle={() => setExpanded((v) => !v)}
          actions={approvalActions || undefined}
        >
          <MarkdownContent content={compactMarkdown(redactSensitiveContent(String(planContent)))} />
        </ChatEntryContainer>
      ) : shouldRenderTodoCard ? (
        <div className="flex flex-col gap-2">
          <ChatTodoList
            todos={todoItems.map((t) => ({ content: t.content, status: t.status }))}
            expanded={expanded}
            onToggle={() => setExpanded((v) => !v)}
          />
          {toolHeaderRight && (
            <div className="flex items-center gap-2">{toolHeaderRight}</div>
          )}
          {approvalActions && (
            <div className="flex items-center gap-1">{approvalActions}</div>
          )}
        </div>
      ) : action === 'command_run' ? (
        <LogLine
          icon={Icon}
          tone="tool"
          content={
            (() => {
              const commandStr = formatShellCommandForDisplay(
                String(actionType.command || '')
              );
              const isLongCommand = commandStr.length > 80 || commandStr.includes('\n');
              const hasExpandableContent = hasCommandDetails || isLongCommand;

              return (
                <div className="flex flex-col w-full max-w-3xl">
                  <div className="rounded-md border border-slate-300 dark:border-zinc-700/50 bg-slate-50 dark:bg-[#1e1e1e] overflow-hidden shadow-sm">
                    <div
                      className={cn(
                        "bg-slate-200/60 dark:bg-[#2d2d2d] px-3 py-1.5 flex items-center justify-between border-b border-slate-300 dark:border-zinc-700/50 transition-colors",
                        hasExpandableContent && "cursor-pointer hover:bg-slate-200/90 dark:hover:bg-[#353535]"
                      )}
                      onClick={() => hasExpandableContent && setExpanded(!expanded)}
                    >
                      <div className="flex items-center gap-2 text-xs font-mono font-medium text-slate-700 dark:text-zinc-300">
                        <Terminal className="h-3.5 w-3.5" />
                        <span>Terminal</span>
                      </div>
                      <div className="flex items-center gap-2">
                        {actionArea}
                        {hasExpandableContent && (
                          <ChevronRight className={cn(
                            "h-3 w-3 text-slate-500 dark:text-zinc-400 transition-transform duration-200 ease-out",
                            expanded && "rotate-90"
                          )} />
                        )}
                      </div>
                    </div>
                    {/* Command - always visible, truncated when collapsed (only if expandable) */}
                    <div className={cn(
                      "px-3 py-2.5 text-[13px] text-slate-900 dark:text-[#cccccc] font-mono leading-relaxed transition-all duration-200 ease-out",
                      hasExpandableContent && !expanded
                        ? "truncate overflow-hidden whitespace-nowrap"
                        : "whitespace-pre-wrap break-all",
                      expanded && hasCommandDetails && "border-b border-slate-300 dark:border-zinc-700/50"
                    )}>
                      <span className="text-slate-400 dark:text-zinc-500 select-none mr-2">$</span>
                      {commandStr}
                    </div>
                    {/* Output - animated expand/collapse */}
                    {hasCommandDetails && commandOutput && (
                      <div
                        className="grid transition-[grid-template-rows] duration-200 ease-out"
                        style={{ gridTemplateRows: expanded ? '1fr' : '0fr' }}
                      >
                        <div className="overflow-hidden">
                          <pre className="p-3 whitespace-pre-wrap break-all text-[13px] text-slate-700 dark:text-[#9cdcfe] font-mono leading-5 overflow-x-auto bg-[#f8fafc] dark:bg-[#1e1e1e] max-h-[300px] overflow-y-auto">
                            {commandOutput}
                          </pre>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              );
            })()
          }
          hideAvatarAndBorder={toolCall.hideAvatarAndBorder}
        />
      ) : hasDetails ? (
        <ChatEntryContainer
          variant="tool"
          icon={Icon}
          title={summary}
          headerRight={toolHeaderRight}
          expanded={expanded}
          onToggle={() => setExpanded((value) => !value)}
          actions={approvalActions || undefined}
        >
          {toolError ? (
            <div className="text-destructive text-sm">{redactSensitiveContent(toolError)}</div>
          ) : null}
        </ChatEntryContainer>
      ) : (action === 'file_read' || action === 'file_edit' || action === 'file_write') &&
        (actionType.file_path || actionType.path) ? (
        <div className="flex flex-col gap-1">
          <ChatFileToolRow
            action={action as 'file_read' | 'file_edit' | 'file_write'}
            path={redactSensitiveContent(actionType.file_path || actionType.path || '')}
            linesAdded={getLinesFromAction(actionType)}
            linesRemoved={getLinesRemovedFromAction(actionType)}
            onViewDiff={
              toolCall.diffId && onViewDiff
                ? () =>
                    onViewDiff(
                      toolCall.diffId!,
                      actionType.file_path || actionType.path
                    )
                : undefined
            }
          />
          {approvalActions && (
            <div className="flex items-center gap-1">{approvalActions}</div>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-1">
          <ChatToolSummary
            summary={summary}
            status={toolStatus}
            actionType={action}
            toolName={toolCall.toolName}
            onViewContent={
              toolCall.diffId && onViewDiff
                ? () =>
                    onViewDiff(
                      toolCall.diffId!,
                      toolCall.actionType?.file_path || toolCall.actionType?.path
                    )
                : undefined
            }
          />
          {approvalActions && (
            <div className="flex items-center gap-1">{approvalActions}</div>
          )}
        </div>
      )}



      {toolError && !(hasDetails && !shouldRenderTodoCard && action !== 'command_run') && (
        <LogLine
          tone="error"
          icon={AlertTriangle}
          content={`Error: ${redactSensitiveContent(toolError)}`}
          detail
          hideAvatarAndBorder={toolCall.hideAvatarAndBorder}
        />
      )}

      {approvalError && (
        <LogLine
          tone="error"
          icon={AlertTriangle}
          content={`Approval error: ${redactSensitiveContent(approvalError)}`}
          detail
          hideAvatarAndBorder={toolCall.hideAvatarAndBorder}
        />
      )}
    </>
  );
}

function OperationGroupRows({ group }: { group: OperationGroup & { hideAvatarAndBorder?: boolean } }) {
  const summary = formatGroupSummary(group);
  return (
    <ChatToolSummary
      summary={summary}
      actionType={group.groupType}
    />
  );
}

/** Inline single thinking (Vibe Kanban ChatThinkingMessage style) */
function ThinkingInline({ content }: { content: string }) {
  return (
    <div className="flex items-start gap-2 text-sm text-muted-foreground">
      <Brain className="shrink-0 pt-0.5 h-4 w-4" />
      <div className="flex-1 min-w-0">
        <MarkdownContent content={compactMarkdown(redactSensitiveContent(content))} />
      </div>
    </div>
  );
}

/** Collapsible thinking group - exported for use in block renderer */
export function ThinkingGroupRenderer({
  entries,
  expanded,
  onToggle,
}: {
  entries: Array<{ content: string; expansionKey: string }>;
  expanded: boolean;
  onToggle: () => void;
}) {
  return (
    <ChatCollapsedThinking
      entries={entries}
      expanded={expanded}
      onToggle={onToggle}
      renderMarkdown={(content) => (
        <MarkdownContent content={compactMarkdown(redactSensitiveContent(content))} />
      )}
    />
  );
}

function LoadingEntry({ text, hideAvatarAndBorder }: { text?: string; hideAvatarAndBorder?: boolean }) {
  const loadingText = text || 'Working...';

  return (
    <div className={cn("flex w-full items-start gap-4 py-4 transition-colors", !hideAvatarAndBorder && "px-4 border-b border-border/40")}>
      {!hideAvatarAndBorder && (
        <div className="flex-shrink-0 mt-0.5">
          <div className="flex h-8 w-8 items-center justify-center rounded-md border bg-transparent border-transparent">
            <LoaderCircle className="h-5 w-5 animate-spin text-muted-foreground/60" />
          </div>
        </div>
      )}
      <div className="flex-1 min-w-0 flex flex-col justify-center py-2 relative overflow-hidden">
        <div className="absolute inset-0 bg-gradient-to-r from-transparent via-muted/10 to-transparent -translate-x-full animate-[shimmer_2s_infinite]" />
        <div className="flex items-center gap-3 relative z-10">
          <LoaderCircle className="h-4 w-4 animate-spin text-primary/70" />
          <span className="text-[13px] font-medium text-muted-foreground animate-pulse">{loadingText}</span>
        </div>
      </div>
    </div>
  );
}

export function TimelineEntryRenderer({
  entry,
  onViewDiff,
  hideAvatarAndBorder,
  onEditUserMessage,
  onResetUserMessage,
  showQueuedBadge,
}: TimelineEntryRendererProps) {
  // Inject trick for children that have their own sub-renders
  if (entry.type === 'tool_call') {
    (entry as any).hideAvatarAndBorder = hideAvatarAndBorder;
  }
  if (entry.type === 'operation_group') {
    (entry as any).hideAvatarAndBorder = hideAvatarAndBorder;
  }
  switch (entry.type) {
    case 'loading': {
      // Show dynamic text based on agent's actual state
      return <LoadingEntry text={entry.text} hideAvatarAndBorder={hideAvatarAndBorder} />;
    }

    case 'assistant_message': {
      const content = redactSensitiveContent(entry.content || '');
      return <MarkdownContent content={compactMarkdown(content)} />;
    }

    case 'user_message': {
      const content = redactSensitiveContent(entry.content || '');
      const hasEditReset = onEditUserMessage || onResetUserMessage;
      const headerRight = hasEditReset ? (
        <div className="flex items-center gap-1">
          {onResetUserMessage && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onResetUserMessage(entry.id);
              }}
              className="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
              aria-label={timelineT.reset}
              title={timelineT.reset}
            >
              <RotateCcw className="h-4 w-4" />
            </button>
          )}
          {onEditUserMessage && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onEditUserMessage(entry.id, content);
              }}
              className="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
              aria-label={timelineT.edit}
              title={timelineT.edit}
            >
              <Pencil className="h-4 w-4" />
            </button>
          )}
        </div>
      ) : undefined;
      return (
        <ChatEntryContainer
          variant="user"
          icon={User}
          title={timelineT.you}
          expanded
          alwaysExpanded
          headerRight={headerRight}
          badge={
            showQueuedBadge ? (
              <span
                className="inline-flex items-center gap-1 rounded-md bg-amber-500/20 px-2 py-0.5 text-xs font-medium text-amber-400 border border-amber-500/30"
                title="Agent will process this when the current task completes"
              >
                <Circle className="h-2.5 w-2.5 animate-pulse" />
                Queued
              </span>
            ) : undefined
          }
        >
          <MarkdownContent content={compactMarkdown(content)} />
        </ChatEntryContainer>
      );
    }

    case 'thinking': {
      return <ThinkingInline content={entry.content || ''} />;
    }

    case 'error': {
      const errorEntry = entry as { error?: string };
      return (
        <ChatErrorMessage content={redactSensitiveContent(errorEntry.error || '')} />
      );
    }

    case 'tool_call': {
      return <ToolCallRows toolCall={entry as ToolCallEntry} onViewDiff={onViewDiff} />;
    }

    case 'operation_group': {
      return <OperationGroupRows group={entry as OperationGroup & { hideAvatarAndBorder?: boolean }} />;
    }

    case 'file_change':
      // Handled by ChatAggregatedFileChanges in block renderer
      return null;

    case 'subagent': {
      const subagent = entry as SubagentEntry;
      const summary = `Started subtask: ${subagent.thread.taskDescription}`;
      return <LogLine icon={Bot} tone="subagent" content={redactSensitiveContent(summary)} hideAvatarAndBorder={hideAvatarAndBorder} />;
    }

    default: {
      return (
        <LogLine icon={AlertTriangle} tone="error" content={`Unknown entry type: ${entry.type}`} hideAvatarAndBorder={hideAvatarAndBorder} />
      );
    }
  }
}
