/**
 * UnifiedView - Display diff in unified (single column) mode
 *
 * Features:
 * - Single column with all changes
 * - Clear add/delete indicators
 * - Syntax highlighting
 * - Context lines
 */

import { memo, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import type { DiffFile, DiffHunk, DiffLine as DiffLineType } from './types';
import { getLanguageFromPath } from './types';
import { UnifiedDiffLine } from './DiffLine';
import { InlineCommentForm } from './InlineCommentForm';

interface UnifiedViewProps {
  file: DiffFile;
  className?: string;
  onAddComment?: (request: { content: string; file_path?: string; line_number?: number }) => Promise<unknown>;
}

interface UnifiedLine {
  type: DiffLineType['type'] | 'hunk-header';
  content: string;
  lineNumber?: number;
}

function buildUnifiedLines(hunks: DiffHunk[]): UnifiedLine[] {
  const result: UnifiedLine[] = [];

  for (const hunk of hunks) {
    // Add hunk header
    result.push({
      type: 'hunk-header',
      content: hunk.header,
    });

    // Add all changes
    for (const change of hunk.changes) {
      result.push({
        type: change.type,
        content: change.content,
        lineNumber: change.type === 'del' ? change.oldLine : change.newLine,
      });
    }
  }

  return result;
}

export const UnifiedView = memo(function UnifiedView({ file, className, onAddComment }: UnifiedViewProps) {
  const language = getLanguageFromPath(file.path);
  const unifiedLines = buildUnifiedLines(file.hunks);
  const [commentLine, setCommentLine] = useState<number | null>(null);

  const handleLineClick = useCallback((lineNum: number | undefined) => {
    if (lineNum && onAddComment) {
      setCommentLine(lineNum);
    }
  }, [onAddComment]);

  const handleAddComment = useCallback(async (content: string) => {
    if (commentLine && onAddComment) {
      await onAddComment({
        content,
        file_path: file.path,
        line_number: commentLine,
      });
      setCommentLine(null);
    }
  }, [commentLine, onAddComment, file.path]);

  if (file.isBinary) {
    return (
      <div className={clsx('p-8 text-center text-muted-foreground', className)}>
        <span className="material-symbols-outlined text-4xl mb-2 block opacity-50">image</span>
        <p>Binary file - cannot display diff</p>
      </div>
    );
  }

  if (unifiedLines.length === 0) {
    return (
      <div className={clsx('p-8 text-center text-muted-foreground', className)}>
        <span className="material-symbols-outlined text-4xl mb-2 block opacity-50">description</span>
        <p>No changes to display</p>
      </div>
    );
  }

  return (
    <div className={clsx('overflow-auto', className)}>
      {unifiedLines.map((line, idx) => {
        if (line.type === 'hunk-header') {
          return (
            <div
              key={idx}
              className="px-3 py-1 bg-muted/40 text-muted-foreground font-mono text-xs border-y border-border"
            >
              {line.content}
            </div>
          );
        }

        const isCommentTarget = commentLine === line.lineNumber;
        return (
          <div key={idx} className="relative">
            <UnifiedDiffLine
              type={line.type}
              content={line.content}
              lineNumber={line.lineNumber}
              language={language}
              onClick={onAddComment ? () => handleLineClick(line.lineNumber) : undefined}
            />
            {isCommentTarget && (
              <InlineCommentForm
                filePath={file.path}
                lineNumber={commentLine}
                onSubmit={handleAddComment}
                onClose={() => setCommentLine(null)}
              />
            )}
          </div>
        );
      })}
    </div>
  );
});
