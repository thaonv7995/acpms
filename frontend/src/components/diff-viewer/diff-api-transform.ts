/**
 * diff-api-transform - Transform API responses to internal diff types
 */

import type { DiffFile, DiffSummary, DiffLine } from './types';

// API response types (snake_case from backend)
export interface ApiDiffResponse {
  files: Array<{
    change: string;
    old_path: string | null;
    new_path: string | null;
    old_content: string | null;
    new_content: string | null;
    additions: number;
    deletions: number;
  }>;
  total_files: number;
  total_additions: number;
  total_deletions: number;
}

function mapApiStatusToStatus(change: string): 'added' | 'modified' | 'deleted' | 'renamed' {
  switch (change) {
    case 'added':
      return 'added';
    case 'deleted':
      return 'deleted';
    case 'renamed':
      return 'renamed';
    default:
      return 'modified';
  }
}

function parseHunksFromContent(
  oldContent: string | null,
  newContent: string | null
): { hunks: DiffFile['hunks']; additions: number; deletions: number } {
  const oldLines = oldContent?.split('\n') || [];
  const newLines = newContent?.split('\n') || [];

  const changes: DiffLine[] = [];
  let additions = 0;
  let deletions = 0;

  const maxLines = Math.max(oldLines.length, newLines.length);

  for (let i = 0; i < maxLines; i++) {
    const oldLine = oldLines[i];
    const newLine = newLines[i];

    if (oldLine === newLine) {
      if (oldLine !== undefined) {
        changes.push({
          type: 'normal',
          oldLine: i + 1,
          newLine: i + 1,
          content: oldLine,
        });
      }
    } else {
      if (oldLine !== undefined) {
        changes.push({
          type: 'del',
          oldLine: i + 1,
          content: oldLine,
        });
        deletions++;
      }
      if (newLine !== undefined) {
        changes.push({
          type: 'add',
          newLine: i + 1,
          content: newLine,
        });
        additions++;
      }
    }
  }

  return {
    hunks:
      changes.length > 0
        ? [
            {
              header: `@@ -1,${oldLines.length} +1,${newLines.length} @@`,
              oldStart: 1,
              oldLines: oldLines.length,
              newStart: 1,
              newLines: newLines.length,
              changes,
            },
          ]
        : [],
    additions,
    deletions,
  };
}

export function transformApiResponse(apiResponse: ApiDiffResponse): {
  files: DiffFile[];
  summary: DiffSummary;
} {
  const files: DiffFile[] = apiResponse.files.map((file) => {
    const path = file.new_path || file.old_path || 'unknown';
    const status = mapApiStatusToStatus(file.change);
    const { hunks } = parseHunksFromContent(file.old_content, file.new_content);

    return {
      path,
      oldPath: file.old_path || undefined,
      status,
      additions: file.additions,
      deletions: file.deletions,
      isBinary: false,
      hunks,
      oldContent: file.old_content || undefined,
      newContent: file.new_content || undefined,
    };
  });

  const summary: DiffSummary = {
    totalFiles: apiResponse.total_files,
    totalAdditions: apiResponse.total_additions,
    totalDeletions: apiResponse.total_deletions,
    filesAdded: files.filter((f) => f.status === 'added').length,
    filesModified: files.filter((f) => f.status === 'modified').length,
    filesDeleted: files.filter((f) => f.status === 'deleted').length,
    filesRenamed: files.filter((f) => f.status === 'renamed').length,
  };

  return { files, summary };
}
