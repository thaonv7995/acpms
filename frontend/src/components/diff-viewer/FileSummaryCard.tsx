/**
 * FileSummaryCard - Shows file count, +/- stats, and file tree
 *
 * Features:
 * - Total files changed count
 * - Additions/deletions stats
 * - Expandable file tree
 * - Click on file to scroll to its diff
 */

import { memo, useState } from 'react';
import { clsx } from 'clsx';
import type { DiffFile, DiffSummary } from './types';
import { STATUS_CONFIG, parseFilePath } from './types';

interface FileSummaryCardProps {
  summary: DiffSummary;
  files: DiffFile[];
  selectedFile?: string;
  onFileSelect?: (path: string) => void;
}

export const FileSummaryCard = memo(function FileSummaryCard({
  summary,
  files,
  selectedFile,
  onFileSelect,
}: FileSummaryCardProps) {
  const [isExpanded, setIsExpanded] = useState(true);

  return (
    <div className="border border-border bg-background overflow-hidden">
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-muted/40 transition-colors"
      >
        <div className="flex items-center gap-3">
          <span className="material-symbols-outlined text-[18px] text-muted-foreground">
            folder_open
          </span>
          <span className="font-medium text-foreground">
            {summary.totalFiles} {summary.totalFiles === 1 ? 'file' : 'files'} changed
          </span>
        </div>

        <div className="flex items-center gap-3">
          {/* Stats */}
          <div className="flex items-center gap-2 text-xs">
            {summary.totalAdditions > 0 && (
              <span className="text-emerald-500 font-medium">+{summary.totalAdditions}</span>
            )}
            {summary.totalDeletions > 0 && (
              <span className="text-red-500 font-medium">-{summary.totalDeletions}</span>
            )}
          </div>

          {/* Expand/collapse icon */}
          <span
            className={clsx(
              'material-symbols-outlined text-[16px] text-muted-foreground transition-transform',
              isExpanded && 'rotate-180'
            )}
          >
            expand_more
          </span>
        </div>
      </button>

      {/* File Tree */}
      {isExpanded && files.length > 0 && (
        <div className="border-t border-border">
          <div className="max-h-[240px] overflow-y-auto">
            {files.map((file) => {
              const config = STATUS_CONFIG[file.status];
              const { fileName, dirPath } = parseFilePath(file.path);
              const isSelected = selectedFile === file.path;

              return (
                <button
                  key={file.path}
                  onClick={() => onFileSelect?.(file.path)}
                  className={clsx(
                    'w-full flex items-center gap-3 px-3 py-2 text-left transition-colors',
                    isSelected
                      ? 'bg-accent/30'
                      : 'hover:bg-muted/30'
                  )}
                >
                  {/* Status icon */}
                  <span className={clsx('material-symbols-outlined text-[16px]', config.color)}>
                    {config.icon}
                  </span>

                  {/* File name */}
                  <div className="flex-1 min-w-0">
                    <span className="font-mono text-sm text-foreground truncate block">
                      {fileName}
                    </span>
                    {dirPath && (
                      <span className="text-xs text-muted-foreground truncate block">
                        {dirPath}
                      </span>
                    )}
                  </div>

                  {/* Stats */}
                  <div className="flex items-center gap-2 text-xs shrink-0">
                    {file.additions > 0 && (
                      <span className="text-green-500 font-medium">+{file.additions}</span>
                    )}
                    {file.deletions > 0 && (
                      <span className="text-red-500 font-medium">-{file.deletions}</span>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* Empty state */}
      {isExpanded && files.length === 0 && (
        <div className="border-t border-border px-4 py-6 text-center text-muted-foreground">
          <span className="material-symbols-outlined text-3xl mb-2 block opacity-50">
            folder_off
          </span>
          <span className="text-sm">No files changed</span>
        </div>
      )}
    </div>
  );
});
