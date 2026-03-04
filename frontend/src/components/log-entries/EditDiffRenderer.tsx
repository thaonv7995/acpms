import { useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { cn } from '@/lib/utils';
import { parseDiff, type DiffLine, type DiffHunk } from '@/utils/diff-parser';

interface EditDiffRendererProps {
  unifiedDiff: string;
  filePath?: string;
}

const MAX_VISIBLE_LINES = 100;

/**
 * Render unified diff format with color-coded changes.
 * Shows line numbers and proper syntax highlighting.
 */
export function EditDiffRenderer({
  unifiedDiff,
  filePath,
}: EditDiffRendererProps) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  const hunks = parseDiff(unifiedDiff);
  const totalLines = hunks.reduce((sum, h) => sum + h.lines.length, 0);
  const isTruncated = totalLines > MAX_VISIBLE_LINES;
  const visibleHunks = expanded ? hunks : truncateHunks(hunks, MAX_VISIBLE_LINES);

  const handleCopy = () => {
    navigator.clipboard.writeText(unifiedDiff);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="space-y-2">
      {/* Header with copy button */}
      <div className="flex items-center justify-between px-3 py-2 bg-muted rounded-t border border-b-0">
        {filePath && (
          <code className="text-xs font-mono text-muted-foreground">
            {filePath}
          </code>
        )}
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          {copied ? (
            <>
              <Check className="w-3 h-3" />
              Copied
            </>
          ) : (
            <>
              <Copy className="w-3 h-3" />
              Copy diff
            </>
          )}
        </button>
      </div>

      {/* Diff content */}
      <div className="font-mono text-xs bg-muted/50 rounded-b border overflow-x-auto">
        {visibleHunks.map((hunk, hunkIdx) => (
          <div key={hunkIdx}>
            {/* Hunk header */}
            <div className="px-3 py-1 bg-muted text-muted-foreground border-b">
              {hunk.header}
            </div>

            {/* Hunk lines */}
            {hunk.lines.map((line, lineIdx) => (
              <DiffLineComponent key={lineIdx} line={line} />
            ))}
          </div>
        ))}
      </div>

      {/* Show more button */}
      {isTruncated && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-xs text-primary hover:underline"
        >
          {expanded
            ? 'Show less'
            : `Show more (${totalLines - MAX_VISIBLE_LINES} lines)`}
        </button>
      )}
    </div>
  );
}

/**
 * Render individual diff line with proper coloring
 */
function DiffLineComponent({ line }: { line: DiffLine }) {
  const isAddition = line.type === 'add';
  const isDeletion = line.type === 'delete';
  const isHunk = line.type === 'hunk';

  return (
    <div
      className={cn(
        'flex',
        isAddition && 'bg-green-500/15 hover:bg-green-500/25',
        isDeletion && 'bg-red-500/15 hover:bg-red-500/25',
        isHunk && 'bg-muted text-muted-foreground',
        'transition-colors'
      )}
    >
      {/* Line number (old) */}
      <div
        className={cn(
          'w-12 flex-shrink-0 text-right pr-2 pl-1 select-none',
          isDeletion ? 'text-red-500/60' : 'text-muted-foreground/60'
        )}
      >
        {line.oldLineNum || ''}
      </div>

      {/* Line number (new) */}
      <div
        className={cn(
          'w-12 flex-shrink-0 text-right pr-2 pl-1 select-none',
          isAddition ? 'text-green-500/60' : 'text-muted-foreground/60'
        )}
      >
        {line.newLineNum || ''}
      </div>

      {/* Change indicator and content */}
      <div className="flex-1 flex min-w-0">
        <span className="w-6 flex-shrink-0 text-center select-none font-bold">
          {line.type === 'add' && (
            <span className="text-green-500">+</span>
          )}
          {line.type === 'delete' && (
            <span className="text-red-500">-</span>
          )}
          {line.type === 'context' && (
            <span className="text-muted-foreground"> </span>
          )}
          {line.type === 'hunk' && (
            <span className="text-muted-foreground">@</span>
          )}
        </span>

        {/* Line content with proper whitespace handling */}
        <code className="flex-1 whitespace-pre-wrap break-words px-1">
          {line.content}
        </code>
      </div>
    </div>
  );
}

/**
 * Truncate hunks to show only first N lines
 */
function truncateHunks(hunks: DiffHunk[], maxLines: number): DiffHunk[] {
  const result: DiffHunk[] = [];
  let lineCount = 0;

  for (const hunk of hunks) {
    if (lineCount >= maxLines) break;

    const remainingLines = maxLines - lineCount;
    if (hunk.lines.length <= remainingLines) {
      result.push(hunk);
      lineCount += hunk.lines.length;
    } else {
      // Truncate this hunk
      result.push({
        ...hunk,
        lines: hunk.lines.slice(0, remainingLines),
      });
      break;
    }
  }

  return result;
}
