import { Bot } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { AssistantMessageEntry } from '@/types/timeline-log';
import { formatTimestamp } from '@/utils/formatters';

interface AssistantMessageCardProps {
  message: AssistantMessageEntry;
}

/**
 * Make inline code clickable if it looks like a URL or file path
 */
function EnhancedCode({ children, ...props }: any) {
  const text = String(children);

  // Check if it's a URL
  if (text.match(/^https?:\/\//)) {
    return (
      <a
        href={text}
        target="_blank"
        rel="noopener noreferrer"
        className="text-[13px] font-medium text-sky-400 hover:text-sky-300 hover:underline cursor-pointer transition-colors"
      >
        {text}
      </a>
    );
  }

  // Check if it's a file path
  if (text.match(/^[./].*\.(tsx?|jsx?|html|css|json|md|rs|toml|yml|yaml)$/i)) {
    return (
      <code className="text-primary hover:underline cursor-pointer" {...props}>
        {children}
      </code>
    );
  }

  return <code {...props}>{children}</code>;
}

/**
 * Enhanced link with better styling
 */
function EnhancedLink({ href, children }: any) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-[13px] font-medium text-sky-400 hover:text-sky-300 hover:underline transition-colors"
    >
      {children}
    </a>
  );
}

/**
 * Assistant message card for timeline.
 * Displays agent responses and status updates with markdown rendering.
 */
export function AssistantMessageCard({ message }: AssistantMessageCardProps) {
  return (
    <div className="relative pl-12">
      {/* Timeline dot */}
      <div
        className="absolute left-[1.875rem] top-3 w-3 h-3 rounded-full border-2 border-background bg-success"
        aria-hidden="true"
      />

      {/* Card */}
      <div className="border border-border rounded-lg overflow-hidden bg-card">
        <div className="px-4 py-3">
          {/* Header */}
          <div className="flex items-center gap-2 mb-2">
            <Bot className="w-4 h-4 text-success" />
            <span className="text-sm font-medium text-success">Assistant</span>
            <span className="text-xs text-muted-foreground">
              {formatTimestamp(message.timestamp)}
            </span>
          </div>

          {/* Message content with markdown */}
          <div className="prose prose-sm prose-compact max-w-none dark:prose-invert
                          prose-headings:text-sm prose-headings:font-medium prose-headings:mt-2 prose-headings:mb-1
                          prose-p:text-xs prose-p:my-1
                          prose-ul:text-xs prose-ul:my-1 prose-li:my-0
                          prose-table:text-xs prose-th:py-1 prose-td:py-1
                          prose-code:text-xs prose-pre:text-xs prose-pre:my-2
                          prose-a:text-sky-400 prose-a:no-underline hover:prose-a:text-sky-300 hover:prose-a:underline">
            {typeof ReactMarkdown !== 'undefined' ? (
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  code: EnhancedCode,
                  a: EnhancedLink,
                }}
              >
                {message.content}
              </ReactMarkdown>
            ) : (
              <div className="text-xs whitespace-pre-wrap">{message.content}</div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
