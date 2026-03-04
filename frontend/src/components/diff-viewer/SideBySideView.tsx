/**
 * SideBySideView - Display diff in side-by-side (split) mode
 *
 * Features:
 * - Old content on left, new content on right
 * - Synchronized scrolling
 * - Line-by-line comparison
 * - Syntax highlighting
 */

import { memo, useRef, useCallback, useState } from 'react';
import { clsx } from 'clsx';
import type { DiffFile, DiffHunk } from './types';
import { getLanguageFromPath } from './types';
import { DiffLine } from './DiffLine';
import { InlineCommentForm } from './InlineCommentForm';

interface SideBySideViewProps {
  file: DiffFile;
  className?: string;
  onAddComment?: (request: { content: string; file_path?: string; line_number?: number }) => Promise<unknown>;
}

interface SplitLine {
  left: { type: 'del' | 'normal' | 'empty'; content: string; lineNum?: number } | null;
  right: { type: 'add' | 'normal' | 'empty'; content: string; lineNum?: number } | null;
}

function buildSplitLines(hunks: DiffHunk[]): SplitLine[] {
  const result: SplitLine[] = [];

  for (const hunk of hunks) {
    // Add hunk header
    result.push({
      left: { type: 'empty', content: hunk.header },
      right: { type: 'empty', content: '' },
    });

    let i = 0;
    while (i < hunk.changes.length) {
      const change = hunk.changes[i];

      if (change.type === 'normal') {
        result.push({
          left: { type: 'normal', content: change.content, lineNum: change.oldLine },
          right: { type: 'normal', content: change.content, lineNum: change.newLine },
        });
        i++;
      } else if (change.type === 'del') {
        // Check if next line is an add (modification)
        const nextChange = hunk.changes[i + 1];
        if (nextChange?.type === 'add') {
          result.push({
            left: { type: 'del', content: change.content, lineNum: change.oldLine },
            right: { type: 'add', content: nextChange.content, lineNum: nextChange.newLine },
          });
          i += 2;
        } else {
          result.push({
            left: { type: 'del', content: change.content, lineNum: change.oldLine },
            right: null,
          });
          i++;
        }
      } else if (change.type === 'add') {
        result.push({
          left: null,
          right: { type: 'add', content: change.content, lineNum: change.newLine },
        });
        i++;
      } else {
        i++;
      }
    }
  }

  return result;
}

export const SideBySideView = memo(function SideBySideView({ file, className, onAddComment }: SideBySideViewProps) {
  const leftPanelRef = useRef<HTMLDivElement>(null);
  const rightPanelRef = useRef<HTMLDivElement>(null);
  const isScrolling = useRef(false);
  const [commentLine, setCommentLine] = useState<{ lineNum: number; side: 'left' | 'right' } | null>(null);

  const language = getLanguageFromPath(file.path);
  const splitLines = buildSplitLines(file.hunks);

  const handleLineClick = useCallback((lineNum: number | undefined, side: 'left' | 'right') => {
    if (lineNum && onAddComment) {
      setCommentLine({ lineNum, side });
    }
  }, [onAddComment]);

  const handleAddComment = useCallback(async (content: string) => {
    if (commentLine && onAddComment) {
      await onAddComment({
        content,
        file_path: file.path,
        line_number: commentLine.lineNum,
      });
      setCommentLine(null);
    }
  }, [commentLine, onAddComment, file.path]);

  const handleScroll = useCallback((source: 'left' | 'right') => {
    if (isScrolling.current) return;
    isScrolling.current = true;

    const sourceRef = source === 'left' ? leftPanelRef : rightPanelRef;
    const targetRef = source === 'left' ? rightPanelRef : leftPanelRef;

    if (sourceRef.current && targetRef.current) {
      targetRef.current.scrollTop = sourceRef.current.scrollTop;
    }

    requestAnimationFrame(() => {
      isScrolling.current = false;
    });
  }, []);

  if (file.isBinary) {
    return (
      <div className={clsx('p-8 text-center text-muted-foreground', className)}>
        <span className="material-symbols-outlined text-4xl mb-2 block opacity-50">image</span>
        <p>Binary file - cannot display diff</p>
      </div>
    );
  }

  if (splitLines.length === 0) {
    return (
      <div className={clsx('p-8 text-center text-muted-foreground', className)}>
        <span className="material-symbols-outlined text-4xl mb-2 block opacity-50">description</span>
        <p>No changes to display</p>
      </div>
    );
  }

  return (
    <div className={clsx('flex overflow-hidden', className)}>
      {/* Left panel (old) */}
      <div
        ref={leftPanelRef}
        className="flex-1 overflow-auto border-r border-border"
        onScroll={() => handleScroll('left')}
      >
        {splitLines.map((line, idx) => {
          if (line.left?.type === 'empty' && line.left.content.startsWith('@@')) {
            return (
              <div
                key={idx}
                className="px-3 py-1 bg-muted/40 text-muted-foreground font-mono text-xs border-b border-border"
              >
                {line.left.content}
              </div>
            );
          }

          if (!line.left) {
            return (
              <div key={idx} className="min-h-[22px] bg-muted/20" />
            );
          }

          const isCommentTarget = commentLine?.side === 'left' && commentLine?.lineNum === line.left.lineNum;
          return (
            <div key={idx} className="relative">
              <DiffLine
                type={line.left.type === 'empty' ? 'normal' : line.left.type}
                content={line.left.content}
                oldLine={line.left.lineNum}
                language={language}
                showOldLine={true}
                showNewLine={false}
                onClick={onAddComment ? () => handleLineClick(line.left?.lineNum, 'left') : undefined}
              />
              {isCommentTarget && (
                <InlineCommentForm
                  filePath={file.path}
                  lineNumber={commentLine.lineNum}
                  onSubmit={handleAddComment}
                  onClose={() => setCommentLine(null)}
                />
              )}
            </div>
          );
        })}
      </div>

      {/* Right panel (new) */}
      <div
        ref={rightPanelRef}
        className="flex-1 overflow-auto"
        onScroll={() => handleScroll('right')}
      >
        {splitLines.map((line, idx) => {
          if (line.left?.type === 'empty' && line.left.content.startsWith('@@')) {
            return (
              <div
                key={idx}
                className="px-3 py-1 bg-muted/40 text-muted-foreground font-mono text-xs border-b border-border"
              >
                {line.left.content}
              </div>
            );
          }

          if (!line.right) {
            return (
              <div key={idx} className="min-h-[22px] bg-muted/20" />
            );
          }

          const isCommentTarget = commentLine?.side === 'right' && commentLine?.lineNum === line.right.lineNum;
          return (
            <div key={idx} className="relative">
              <DiffLine
                type={line.right.type === 'empty' ? 'normal' : line.right.type}
                content={line.right.content}
                newLine={line.right.lineNum}
                language={language}
                showOldLine={false}
                showNewLine={true}
                onClick={onAddComment ? () => handleLineClick(line.right?.lineNum, 'right') : undefined}
              />
              {isCommentTarget && (
                <InlineCommentForm
                  filePath={file.path}
                  lineNumber={commentLine.lineNum}
                  onSubmit={handleAddComment}
                  onClose={() => setCommentLine(null)}
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});
