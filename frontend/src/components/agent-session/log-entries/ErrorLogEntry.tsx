/**
 * ErrorLogEntry - Displays error messages with red background
 * Shows error details and optional stack trace
 */

import { memo, useState } from 'react';
import type { AgentLogEntry } from '../types';

interface ErrorLogEntryProps {
  entry: AgentLogEntry;
}

export const ErrorLogEntry = memo(function ErrorLogEntry({ entry }: ErrorLogEntryProps) {
  const [showStack, setShowStack] = useState(false);
  const hasStack = entry.metadata?.stack_trace;

  return (
    <div className="bg-red-500/10 border border-red-500/30 rounded-md px-3 py-2 my-1">
      <div className="flex items-start gap-2">
        <span className="material-symbols-outlined text-[18px] text-red-400 shrink-0 mt-0.5">
          error
        </span>
        <div className="flex-1 min-w-0">
          <p className="text-sm text-red-300 whitespace-pre-wrap break-words">
            {entry.content}
          </p>

          {entry.metadata?.error_code && (
            <span className="inline-block mt-1 px-1.5 py-0.5 bg-red-500/20 rounded text-xs text-red-400 font-mono">
              {entry.metadata.error_code}
            </span>
          )}

          {hasStack && (
            <button
              onClick={() => setShowStack(!showStack)}
              className="flex items-center gap-1 mt-2 text-xs text-red-400/70 hover:text-red-400 transition-colors"
            >
              <span className="material-symbols-outlined text-[14px]">
                {showStack ? 'expand_less' : 'expand_more'}
              </span>
              {showStack ? 'Hide' : 'Show'} stack trace
            </button>
          )}

          {showStack && hasStack && (
            <pre className="mt-2 p-2 bg-slate-900/50 rounded text-xs text-red-400/80 font-mono overflow-x-auto max-h-48 overflow-y-auto">
              {entry.metadata?.stack_trace}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
});
