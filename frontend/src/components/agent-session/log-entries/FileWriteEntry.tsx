/**
 * FileWriteEntry - Displays file write/edit operations
 * Shows filename with diff stats (+N -M) and edit icon
 */

import { memo } from 'react';
import type { AgentLogEntry } from '../types';

interface FileWriteEntryProps {
  entry: AgentLogEntry;
  onFileClick?: (filepath: string) => void;
}

export const FileWriteEntry = memo(function FileWriteEntry({ entry, onFileClick }: FileWriteEntryProps) {
  const filepath = entry.metadata?.filepath || entry.content;
  const additions = entry.metadata?.additions || 0;
  const deletions = entry.metadata?.deletions || 0;
  const isNewFile = entry.metadata?.is_new_file;

  const handleClick = () => {
    if (onFileClick && filepath) {
      onFileClick(filepath);
    }
  };

  return (
    <div className="flex items-center gap-2 py-1 group">
      <span className="text-lg shrink-0" title="File write">
        <span className="material-symbols-outlined text-[18px] text-amber-400">edit_document</span>
      </span>
      <button
        onClick={handleClick}
        className="font-mono text-sm text-amber-400 hover:text-amber-300 hover:underline underline-offset-2 transition-colors truncate"
        title={`View changes in ${filepath}`}
      >
        {filepath}
      </button>

      {/* Diff stats */}
      <div className="flex items-center gap-1.5 text-xs shrink-0">
        {isNewFile ? (
          <span className="px-1.5 py-0.5 rounded bg-green-500/20 text-green-400 font-medium">
            new
          </span>
        ) : (
          <>
            {additions > 0 && (
              <span className="text-green-400 font-mono">+{additions}</span>
            )}
            {deletions > 0 && (
              <span className="text-red-400 font-mono">-{deletions}</span>
            )}
          </>
        )}
      </div>

      <span className="material-symbols-outlined text-[14px] text-slate-500 opacity-0 group-hover:opacity-100 transition-opacity">
        difference
      </span>
    </div>
  );
});
