import { describe, expect, it } from 'vitest';
import { resolveFileDiffForPath } from '../../../hooks/useTimelineStream';
import type { FileDiffSummary } from '../../../types/timeline-log';

function buildDiff(
  file_path: string,
  additions: number,
  deletions: number,
  id: string
): FileDiffSummary {
  return {
    id,
    file_path,
    additions,
    deletions,
    change_type: 'modified',
  };
}

describe('resolveFileDiffForPath', () => {
  it('matches exact path', () => {
    const diffs = [buildDiff('frontend/src/App.tsx', 10, 2, 'd1')];
    const resolved = resolveFileDiffForPath('frontend/src/App.tsx', diffs);
    expect(resolved?.id).toBe('d1');
  });

  it('matches when timeline path is shorter suffix', () => {
    const diffs = [buildDiff('frontend/src/components/timeline-log/TimelineEntryRenderer.tsx', 6, 26, 'd2')];
    const resolved = resolveFileDiffForPath('TimelineEntryRenderer.tsx', diffs);
    expect(resolved?.id).toBe('d2');
    expect(resolved?.additions).toBe(6);
    expect(resolved?.deletions).toBe(26);
  });

  it('matches normalized git-style prefixes', () => {
    const diffs = [buildDiff('a/frontend/src/hooks/useTimelineStream.ts', 11, 0, 'd3')];
    const resolved = resolveFileDiffForPath('./frontend/src/hooks/useTimelineStream.ts', diffs);
    expect(resolved?.id).toBe('d3');
  });

  it('returns null for ambiguous basename matches', () => {
    const diffs = [
      buildDiff('frontend/src/A/README.md', 1, 1, 'd4'),
      buildDiff('frontend/src/B/README.md', 2, 2, 'd5'),
    ];
    const resolved = resolveFileDiffForPath('README.md', diffs);
    expect(resolved).toBeNull();
  });
});
