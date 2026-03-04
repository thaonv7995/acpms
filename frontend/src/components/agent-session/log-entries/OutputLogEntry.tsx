/**
 * OutputLogEntry - Displays general output/stdout
 * Gray indented text for command output, tool results, etc.
 */

import { memo } from 'react';
import type { AgentLogEntry } from '../types';

interface OutputLogEntryProps {
  entry: AgentLogEntry;
}

export const OutputLogEntry = memo(function OutputLogEntry({ entry }: OutputLogEntryProps) {
  const lines = entry.content.split('\n');
  const isLongOutput = lines.length > 20;

  return (
    <div className="ml-4 pl-3 border-l-2 border-slate-700/50 py-1">
      <pre
        className={`font-mono text-xs text-slate-400 whitespace-pre-wrap break-all ${
          isLongOutput ? 'max-h-60 overflow-y-auto' : ''
        }`}
      >
        {entry.content}
      </pre>
      {isLongOutput && (
        <div className="text-xs text-slate-500 mt-1">
          {lines.length} lines of output
        </div>
      )}
    </div>
  );
});
