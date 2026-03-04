/**
 * ThinkingLogEntry - Displays agent thinking/reasoning
 * Italic cyan text with brain icon
 */

import { memo } from 'react';
import type { AgentLogEntry } from '../types';

interface ThinkingLogEntryProps {
  entry: AgentLogEntry;
}

export const ThinkingLogEntry = memo(function ThinkingLogEntry({ entry }: ThinkingLogEntryProps) {
  return (
    <div className="flex items-start gap-2 py-1.5 opacity-80">
      <span className="material-symbols-outlined text-[16px] text-cyan-500 mt-0.5 shrink-0">
        psychology
      </span>
      <p className="text-sm text-cyan-400/90 italic whitespace-pre-wrap break-words leading-relaxed">
        {entry.content}
      </p>
    </div>
  );
});
