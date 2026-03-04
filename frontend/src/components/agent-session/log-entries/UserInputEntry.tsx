/**
 * UserInputEntry - Displays user follow-up messages
 * Blue background to distinguish from agent responses
 */

import { memo } from 'react';
import type { AgentLogEntry } from '../types';

interface UserInputEntryProps {
  entry: AgentLogEntry;
}

export const UserInputEntry = memo(function UserInputEntry({ entry }: UserInputEntryProps) {
  const timestamp = entry.timestamp
    ? new Date(entry.timestamp).toLocaleTimeString('en-US', {
        hour: '2-digit',
        minute: '2-digit',
        hour12: false,
      })
    : null;

  return (
    <div className="flex justify-end py-2">
      <div className="max-w-[85%]">
        <div className="bg-primary/20 border border-primary/30 rounded-lg px-4 py-2.5">
          <div className="flex items-center gap-2 mb-1">
            <span className="material-symbols-outlined text-[14px] text-primary">person</span>
            <span className="text-xs font-medium text-primary">You</span>
            {timestamp && (
              <span className="text-xs text-slate-500">{timestamp}</span>
            )}
          </div>
          <p className="text-sm text-slate-200 whitespace-pre-wrap break-words">
            {entry.content}
          </p>
        </div>
      </div>
    </div>
  );
});
