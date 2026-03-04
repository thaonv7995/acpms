/**
 * ToolCallEntry - Displays tool invocations
 * Shows tool name with expandable input/output
 */

import { memo, useState } from 'react';
import type { AgentLogEntry } from '../types';

interface ToolCallEntryProps {
  entry: AgentLogEntry;
}

export const ToolCallEntry = memo(function ToolCallEntry({ entry }: ToolCallEntryProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const toolName = entry.metadata?.tool_name || 'Unknown Tool';
  const hasDetails = entry.metadata?.tool_input || entry.metadata?.tool_output;
  const duration = entry.metadata?.duration_ms;

  // Get icon based on tool type
  const getToolIcon = (name: string) => {
    const lower = name.toLowerCase();
    if (lower.includes('bash') || lower.includes('command')) return 'terminal';
    if (lower.includes('read')) return 'visibility';
    if (lower.includes('write') || lower.includes('edit')) return 'edit';
    if (lower.includes('search') || lower.includes('grep') || lower.includes('glob')) return 'search';
    if (lower.includes('web') || lower.includes('fetch')) return 'language';
    return 'build';
  };

  return (
    <div className="group py-1">
      <div className="flex items-center gap-2">
        <span className="material-symbols-outlined text-[16px] text-green-400">
          {getToolIcon(toolName)}
        </span>
        <button
          onClick={() => hasDetails && setIsExpanded(!isExpanded)}
          className={`flex items-center gap-2 text-sm font-medium ${
            hasDetails
              ? 'text-green-400 hover:text-green-300 cursor-pointer'
              : 'text-green-400 cursor-default'
          }`}
        >
          <span className="font-mono">{toolName}</span>
          {entry.content && (
            <span className="text-slate-400 font-normal truncate max-w-xs">
              {entry.content}
            </span>
          )}
        </button>
        {duration && (
          <span className="text-xs text-slate-500">{duration}ms</span>
        )}
        {hasDetails && (
          <span className="material-symbols-outlined text-[14px] text-slate-500 opacity-0 group-hover:opacity-100 transition-opacity">
            {isExpanded ? 'expand_less' : 'expand_more'}
          </span>
        )}
      </div>

      {isExpanded && hasDetails && (
        <div className="mt-2 ml-6 space-y-2">
          {entry.metadata?.tool_input && (
            <div>
              <div className="text-xs text-slate-500 mb-1">Input:</div>
              <pre className="p-2 bg-slate-800/50 rounded text-xs text-slate-400 font-mono overflow-x-auto max-h-32 overflow-y-auto">
                {typeof entry.metadata.tool_input === 'string'
                  ? entry.metadata.tool_input
                  : JSON.stringify(entry.metadata.tool_input, null, 2)}
              </pre>
            </div>
          )}
          {entry.metadata?.tool_output && (
            <div>
              <div className="text-xs text-slate-500 mb-1">Output:</div>
              <pre className="p-2 bg-slate-800/50 rounded text-xs text-slate-400 font-mono overflow-x-auto max-h-48 overflow-y-auto">
                {entry.metadata.tool_output}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
});
