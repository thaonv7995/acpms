/**
 * FileReadEntry - Displays file read operations with file icon
 * Shows filename as clickable link to navigate to file viewer
 */

import { memo } from 'react';
import type { AgentLogEntry } from '../types';

interface FileReadEntryProps {
  entry: AgentLogEntry;
  onFileClick?: (filepath: string) => void;
}

export const FileReadEntry = memo(function FileReadEntry({ entry, onFileClick }: FileReadEntryProps) {
  const filepath = entry.metadata?.filepath || entry.content;

  const handleClick = () => {
    if (onFileClick && filepath) {
      onFileClick(filepath);
    }
  };

  return (
    <div className="flex items-center gap-2 py-1 group">
      <span className="text-lg shrink-0" title="File read">
        <span className="material-symbols-outlined text-[18px] text-slate-400">description</span>
      </span>
      <button
        onClick={handleClick}
        className="font-mono text-sm text-cyan-400 hover:text-cyan-300 hover:underline underline-offset-2 transition-colors truncate"
        title={`View ${filepath}`}
      >
        {filepath}
      </button>
      {entry.metadata?.lines && (
        <span className="text-xs text-slate-500 shrink-0">
          ({entry.metadata.lines} lines)
        </span>
      )}
      <span className="material-symbols-outlined text-[14px] text-slate-500 opacity-0 group-hover:opacity-100 transition-opacity">
        open_in_new
      </span>
    </div>
  );
});
