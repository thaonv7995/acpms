/**
 * CommandLogEntry - Displays command execution with green dot indicator
 * Shows the command in monospace font with optional output
 */

import { memo, useState } from 'react';
import type { AgentLogEntry } from '../types';

interface CommandLogEntryProps {
  entry: AgentLogEntry;
}

export const CommandLogEntry = memo(function CommandLogEntry({ entry }: CommandLogEntryProps) {
  const [isOutputExpanded, setIsOutputExpanded] = useState(false);
  const hasOutput = entry.metadata?.output && entry.metadata.output.length > 0;
  const isMultiLine = entry.content.includes('\n');

  return (
    <div className="group">
      {/* Command line */}
      <div className="flex items-start gap-2 py-1">
        <span className="mt-1.5 w-2 h-2 rounded-full bg-green-500 shrink-0" title="Command" />
        <div className="flex-1 min-w-0">
          {isMultiLine ? (
            <pre className="font-mono text-sm text-slate-200 whitespace-pre-wrap break-all">
              {entry.content.split('\n').map((line, i) => (
                <span key={i} className="block">
                  {i === 0 && <span className="text-slate-500 select-none">$ </span>}
                  {i > 0 && <span className="text-slate-500 select-none">&gt; </span>}
                  {line}
                </span>
              ))}
            </pre>
          ) : (
            <code className="font-mono text-sm text-slate-200">
              <span className="text-slate-500 select-none">$ </span>
              {entry.content}
            </code>
          )}
        </div>
        {hasOutput && (
          <button
            onClick={() => setIsOutputExpanded(!isOutputExpanded)}
            className="p-1 rounded hover:bg-slate-700/50 text-slate-400 opacity-0 group-hover:opacity-100 transition-opacity"
            title={isOutputExpanded ? 'Collapse output' : 'Expand output'}
          >
            <span className="material-symbols-outlined text-[16px]">
              {isOutputExpanded ? 'expand_less' : 'expand_more'}
            </span>
          </button>
        )}
      </div>

      {/* Command output (collapsible) */}
      {hasOutput && isOutputExpanded && (
        <div className="ml-4 pl-3 border-l-2 border-slate-700 mt-1 mb-2">
          <pre className="font-mono text-xs text-slate-400 whitespace-pre-wrap break-all max-h-60 overflow-y-auto">
            {entry.metadata?.output}
          </pre>
        </div>
      )}

      {/* Duration indicator */}
      {entry.metadata?.duration_ms && (
        <div className="ml-4 text-xs text-slate-500">
          Completed in {entry.metadata.duration_ms}ms
        </div>
      )}
    </div>
  );
});
