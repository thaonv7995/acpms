export type DiffLineType = 'add' | 'delete' | 'context' | 'hunk';

export interface DiffLine {
  type: DiffLineType;
  content: string;
  oldLineNum?: number;
  newLineNum?: number;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

/**
 * Parse unified diff format into structured hunks and lines
 *
 * Format example:
 * --- a/file.txt
 * +++ b/file.txt
 * @@ -10,5 +10,6 @@
 *  context line
 * -deleted line
 * +added line
 *  context line
 */
export function parseDiff(unifiedDiff: string): DiffHunk[] {
  const lines = unifiedDiff.split('\n');
  const hunks: DiffHunk[] = [];
  let currentHunk: DiffHunk | null = null;
  let oldLineNum = 0;
  let newLineNum = 0;

  for (const line of lines) {
    // Skip file headers
    if (line.startsWith('---') || line.startsWith('+++')) {
      continue;
    }

    // Hunk header
    if (line.startsWith('@@')) {
      if (currentHunk) {
        hunks.push(currentHunk);
      }

      // Parse hunk header: @@ -10,5 +10,6 @@
      const headerMatch = line.match(/@@ -(\d+),?\d* \+(\d+),?\d* @@/);
      if (headerMatch) {
        oldLineNum = parseInt(headerMatch[1], 10);
        newLineNum = parseInt(headerMatch[2], 10);
      }

      currentHunk = {
        header: line,
        lines: [],
      };
      continue;
    }

    if (!currentHunk) continue;

    // Parse diff line
    if (line.startsWith('+') && !line.startsWith('+++')) {
      currentHunk.lines.push({
        type: 'add',
        content: line.slice(1),
        newLineNum: newLineNum++,
      });
    } else if (line.startsWith('-') && !line.startsWith('---')) {
      currentHunk.lines.push({
        type: 'delete',
        content: line.slice(1),
        oldLineNum: oldLineNum++,
      });
    } else {
      // Context line (space prefix)
      const content = line.startsWith(' ') ? line.slice(1) : line;
      currentHunk.lines.push({
        type: 'context',
        content,
        oldLineNum: oldLineNum++,
        newLineNum: newLineNum++,
      });
    }
  }

  if (currentHunk) {
    hunks.push(currentHunk);
  }

  return hunks;
}

/**
 * Calculate diff statistics (additions, deletions)
 */
export function getDiffStats(hunks: DiffHunk[]): {
  additions: number;
  deletions: number;
} {
  let additions = 0;
  let deletions = 0;

  for (const hunk of hunks) {
    for (const line of hunk.lines) {
      if (line.type === 'add') additions++;
      if (line.type === 'delete') deletions++;
    }
  }

  return { additions, deletions };
}

/**
 * Generate plain text unified diff from structured hunks
 */
export function generateUnifiedDiff(
  filePath: string,
  hunks: DiffHunk[]
): string {
  const lines: string[] = [];
  lines.push(`--- a/${filePath}`);
  lines.push(`+++ b/${filePath}`);

  for (const hunk of hunks) {
    lines.push(hunk.header);
    for (const line of hunk.lines) {
      if (line.type === 'add') {
        lines.push('+' + line.content);
      } else if (line.type === 'delete') {
        lines.push('-' + line.content);
      } else {
        lines.push(' ' + line.content);
      }
    }
  }

  return lines.join('\n');
}
