/**
 * SystemLogEntry - Displays system messages with gray background
 * Used for agent start/stop, hook responses, model info, etc.
 */

import { memo } from 'react';
import type { AgentLogEntry } from '../types';

interface SystemLogEntryProps {
  entry: AgentLogEntry;
}

export const SystemLogEntry = memo(function SystemLogEntry({ entry }: SystemLogEntryProps) {
  return (
    <div className="bg-slate-800/50 dark:bg-slate-800/70 rounded-md px-3 py-2 border border-slate-700/50">
      <div className="flex items-start gap-2">
        <span className="text-purple-400 font-medium text-xs uppercase shrink-0">System:</span>
        <span className="text-slate-300 text-sm whitespace-pre-wrap break-words">
          {entry.content}
        </span>
      </div>
      {entry.metadata?.model && (
        <div className="mt-1.5 text-xs text-slate-500">
          Model: <span className="text-slate-400 font-mono">{entry.metadata.model}</span>
        </div>
      )}
    </div>
  );
});
