/**
 * DiffContentArea - Container for displaying file diffs
 *
 * Features:
 * - File header with name and stats
 * - View mode toggle per file
 * - Expandable/collapsible
 * - Copy and link actions
 */

import { memo, useState, useRef, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import type { DiffFile, ViewMode } from './types';
import { STATUS_CONFIG, parseFilePath } from './types';
import { ViewModeToggle } from './ViewModeToggle';
import { SideBySideView } from './SideBySideView';
import { UnifiedView } from './UnifiedView';

interface DiffContentAreaProps {
  files: DiffFile[];
  selectedFile?: string;
  defaultViewMode?: ViewMode;
  onFileSelect?: (path: string) => void;
  attemptId?: string;
  expandAllSignal?: number;
  forceExpanded?: boolean;
  onAddComment?: (request: { content: string; file_path?: string; line_number?: number }) => Promise<unknown>;
}

interface FileCardProps {
  file: DiffFile;
  isSelected: boolean;
  defaultViewMode: ViewMode;
  expandAllSignal: number;
  forceExpanded: boolean;
  onSelect?: () => void;
  onRef?: (el: HTMLDivElement | null) => void;
  onAddComment?: (request: { content: string; file_path?: string; line_number?: number }) => Promise<unknown>;
}

const FileCard = memo(function FileCard({
  file,
  isSelected,
  defaultViewMode,
  expandAllSignal,
  forceExpanded,
  onSelect,
  onRef,
  onAddComment,
}: FileCardProps) {
  const [isExpanded, setIsExpanded] = useState(true);
  const [viewMode, setViewMode] = useState<ViewMode>(defaultViewMode);

  const config = STATUS_CONFIG[file.status];
  const { fileName, dirPath } = parseFilePath(file.path);

  const handleCopyPath = () => {
    navigator.clipboard.writeText(file.path);
  };

  useEffect(() => {
    setIsExpanded(forceExpanded);
  }, [expandAllSignal, forceExpanded]);

  return (
    <div
      ref={onRef}
      className={clsx(
        'border border-border overflow-hidden transition-colors',
        isSelected
          ? 'border-primary/60 bg-muted/20'
          : 'bg-background'
      )}
    >
      {/* File header */}
      <div
        className={clsx(
          'flex items-center gap-2.5 px-3 py-2 bg-muted/30 cursor-pointer',
          isExpanded && 'border-b border-border'
        )}
        onClick={() => {
          setIsExpanded(!isExpanded);
          onSelect?.();
        }}
      >
        {/* Expand/collapse */}
        <span
          className={clsx(
            'material-symbols-outlined text-[16px] text-muted-foreground transition-transform',
            isExpanded && 'rotate-90'
          )}
        >
          chevron_right
        </span>

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
          {file.additions > 0 && <span className="text-emerald-500 font-medium">+{file.additions}</span>}
          {file.deletions > 0 && <span className="text-red-500 font-medium">-{file.deletions}</span>}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          <button
            onClick={handleCopyPath}
            className="h-7 w-7 inline-flex items-center justify-center rounded-sm border border-border text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
            title="Copy file path"
          >
            <span className="material-symbols-outlined text-[16px]">content_copy</span>
          </button>
          <ViewModeToggle mode={viewMode} onChange={setViewMode} />
        </div>
      </div>

      {/* Diff content */}
      {isExpanded && (
        <div className="max-h-[520px] overflow-auto bg-background">
          {viewMode === 'side-by-side' ? (
            <SideBySideView file={file} onAddComment={onAddComment} />
          ) : (
            <UnifiedView file={file} onAddComment={onAddComment} />
          )}
        </div>
      )}
    </div>
  );
});

export const DiffContentArea = memo(function DiffContentArea({
  files,
  selectedFile,
  defaultViewMode = 'side-by-side',
  onFileSelect,
  expandAllSignal = 0,
  forceExpanded = true,
  onAddComment,
}: DiffContentAreaProps) {
  const fileRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  // Scroll to selected file
  useEffect(() => {
    if (selectedFile) {
      const element = fileRefs.current.get(selectedFile);
      if (element) {
        element.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    }
  }, [selectedFile]);

  const createRefCallback = useCallback(
    (path: string) => (el: HTMLDivElement | null) => {
      if (el) {
        fileRefs.current.set(path, el);
      } else {
        fileRefs.current.delete(path);
      }
    },
    []
  );

  if (files.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <div className="text-center">
          <span className="material-symbols-outlined text-5xl mb-3 block opacity-50">
            difference
          </span>
          <p className="text-lg font-medium">No changes to display</p>
          <p className="text-sm mt-1">The agent hasn't made any file changes yet.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {files.map((file) => (
        <FileCard
          key={file.path}
          file={file}
          isSelected={selectedFile === file.path}
          defaultViewMode={defaultViewMode}
          expandAllSignal={expandAllSignal}
          forceExpanded={forceExpanded}
          onSelect={() => onFileSelect?.(file.path)}
          onRef={createRefCallback(file.path)}
          onAddComment={onAddComment}
        />
      ))}
    </div>
  );
});
