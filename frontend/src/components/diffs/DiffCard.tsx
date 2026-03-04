// DiffCard - Expandable file diff card with syntax highlighting
import { useState, useMemo } from 'react';
import { DiffView, DiffModeEnum } from '@git-diff-view/react';
import { DiffFile } from '@git-diff-view/file';
import '@git-diff-view/react/styles/diff-view.css';
import type { FileDiff } from '../../types/diff';
import { statusConfig, getLanguageFromPath, parseFilePath } from './diff-utils';
import { logger } from '@/lib/logger';

interface DiffCardProps {
  diff: FileDiff;
  defaultExpanded?: boolean;
  projectName?: string;
}

export function DiffCard({ diff, defaultExpanded = false, projectName }: DiffCardProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const [viewMode, setViewMode] = useState<DiffModeEnum>(DiffModeEnum.Split);

  const config = statusConfig[diff.status];
  const { fileName, dirPath } = parseFilePath(diff.file_path);

  // Create DiffFile instance for @git-diff-view
  const diffFile = useMemo(() => {
    if (!diff.old_content && !diff.new_content) return null;

    try {
      const lang = getLanguageFromPath(diff.file_path);
      // DiffFile expects: oldFileName, oldContent, newFileName, newContent, diffList[], oldLang, newLang
      const diffLines = diff.hunks.flatMap((h) => h.content.split('\n'));
      const file = new DiffFile(
        diff.old_path || fileName,
        diff.old_content || '',
        fileName,
        diff.new_content || '',
        diffLines,
        lang,
        lang
      );
      file.init();
      file.buildSplitDiffLines();
      file.buildUnifiedDiffLines();
      return file;
    } catch (e) {
      logger.error('Failed to create DiffFile:', e);
      return null;
    }
  }, [diff, fileName]);

  return (
    <div className="border-b border-border">
      {/* File Header */}
      <button
        className={`w-full flex items-center gap-2 px-4 py-2 pl-8 hover:bg-muted/50 transition-colors text-left ${
          isExpanded ? config.bgColor : ''
        }`}
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <span
          className={`material-symbols-outlined text-[16px] text-muted-foreground transition-transform ${
            isExpanded ? 'rotate-90' : ''
          }`}
        >
          chevron_right
        </span>
        <span className="material-symbols-outlined text-[16px] text-muted-foreground">
          edit
        </span>
        <div className="flex-1 min-w-0">
          <span className="font-mono text-sm text-card-foreground truncate block">
            {projectName ? `${projectName}/` : ''}{dirPath ? `${dirPath}/` : ''}{fileName}
          </span>
        </div>
        <div className="flex items-center gap-3 text-xs shrink-0">
          {diff.additions > 0 && (
            <span className="text-green-500 font-medium">+{diff.additions}</span>
          )}
          {diff.deletions > 0 && (
            <span className="text-red-500 font-medium">-{diff.deletions}</span>
          )}
          <span className="material-symbols-outlined text-[16px] text-muted-foreground hover:text-card-foreground cursor-pointer">
            open_in_new
          </span>
        </div>
      </button>

      {/* Diff Content */}
      {isExpanded && (
        <div className="border-t border-border">
          {/* View Mode Toggle */}
          <div className="flex items-center justify-between px-4 py-2 bg-muted/50 border-b border-border">
            <span className="text-xs text-muted-foreground">
              {config.label} file
              {diff.old_path && diff.status === 'renamed' && (
                <span className="ml-2">
                  from <code className="text-card-foreground">{diff.old_path}</code>
                </span>
              )}
            </span>
            <div className="flex items-center gap-1">
              <button
                className={`px-2 py-1 text-xs rounded transition-colors ${
                  viewMode === DiffModeEnum.Split
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted text-card-foreground hover:bg-muted/80'
                }`}
                onClick={() => setViewMode(DiffModeEnum.Split)}
              >
                Split
              </button>
              <button
                className={`px-2 py-1 text-xs rounded transition-colors ${
                  viewMode === DiffModeEnum.Unified
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted text-card-foreground hover:bg-muted/80'
                }`}
                onClick={() => setViewMode(DiffModeEnum.Unified)}
              >
                Unified
              </button>
            </div>
          </div>

          {/* Diff View */}
          <div className="max-h-[500px] overflow-auto">
            {diff.is_binary ? (
              <div className="p-4 text-center text-muted-foreground">
                <span className="material-symbols-outlined text-3xl mb-2 block">image</span>
                Binary file - cannot display diff
              </div>
            ) : diffFile ? (
              <DiffView
                diffFile={diffFile}
                diffViewMode={viewMode}
                diffViewHighlight
                diffViewWrap={false}
              />
            ) : (
              <div className="p-4 text-center text-muted-foreground">
                <span className="material-symbols-outlined text-3xl mb-2 block">code_off</span>
                No diff content available
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
