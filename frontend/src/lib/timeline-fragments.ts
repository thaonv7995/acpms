/**
 * R5: Combine consecutive text fragments into coherent messages.
 * Extracted from useTimelineStream for single-layer parse flow.
 */
import type { TimelineEntry } from '@/types/timeline-log';

/**
 * Combine consecutive assistant_message entries (stdout/sdk fragments) into single messages.
 */
export function combineTextFragments(entries: TimelineEntry[]): TimelineEntry[] {
  const result: TimelineEntry[] = [];
  let textBuffer: string[] = [];
  let bufferStart: TimelineEntry | null = null;
  let bufferSource: string | null = null;
  let bufferLastTimestampMs: number | null = null;

  const suffixPrefixOverlap = (prev: string, next: string): number => {
    const maxOverlap = Math.min(prev.length, next.length);
    for (let len = maxOverlap; len > 0; len -= 1) {
      if (prev.endsWith(next.slice(0, len))) {
        return len;
      }
    }
    return 0;
  };

  const mergeContinuousStreamFragment = (
    prev: string,
    next: string
  ): { content: string; shouldFlush: boolean } => {
    if (!prev) return { content: next, shouldFlush: false };
    if (!next) return { content: prev, shouldFlush: false };

    // Snapshot updates: provider sends full message repeatedly with increasing length.
    if (next.startsWith(prev)) {
      return { content: next, shouldFlush: false };
    }

    // Late/stale update: keep the longer known snapshot.
    if (prev.startsWith(next)) {
      return { content: prev, shouldFlush: false };
    }

    // Delta mode with overlap (suffix/prefix) => append only unseen tail.
    const overlap = suffixPrefixOverlap(prev, next);
    if (overlap >= 2) {
      return { content: prev + next.slice(overlap), shouldFlush: false };
    }

    // Heuristic split for back-to-back messages in the same stream source.
    const prevTrimmed = prev.trimEnd();
    const nextTrimmed = next.trimStart();
    const prevLooksComplete = /[.!?][)"'\]]?$/.test(prevTrimmed);
    const nextLooksNewMessage = /^[A-Z0-9]/.test(nextTrimmed);
    if (prevLooksComplete && nextLooksNewMessage) {
      return { content: next, shouldFlush: true };
    }

    return { content: prev + next, shouldFlush: false };
  };

  const isStdoutTranscriptMarker = (text: string): boolean => {
    const trimmed = (text || '').trimStart();
    if (!trimmed) return false;

    return (
      /^Using tool:\s+/i.test(trimmed) ||
      /^[✓✗]\s+/.test(trimmed) ||
      /^(Created|Modified|Deleted|Renamed):\s+/i.test(trimmed)
    );
  };

  const joinFragments = (fragments: string[]): string => {
    if (fragments.length <= 1) return fragments[0] || '';
    if (bufferSource === 'sdk' || bufferSource === 'normalized') {
      return fragments.join('');
    }

    let out = '';
    for (const fragment of fragments) {
      if (!fragment) continue;
      if (!out) {
        out = fragment;
        continue;
      }

      const needsSeparator = !out.endsWith('\n') && !fragment.startsWith('\n');
      if (needsSeparator) {
        if (/^[ \t]/.test(fragment)) {
          // no-op
        } else {
          out += '\n';
        }
      }
      out += fragment;
    }
    return out;
  };

  for (const entry of entries) {
    const isMergeableAssistant =
      entry.type === 'assistant_message' &&
      entry.content &&
      (entry.source === 'stdout' || entry.source === 'sdk' || entry.source === 'normalized' || !entry.source);

    if (isMergeableAssistant) {
      if (isStdoutTranscriptMarker(entry.content)) {
        if (textBuffer.length > 0 && bufferStart) {
          result.push({
            ...bufferStart,
            content: joinFragments(textBuffer),
          });
        }
        textBuffer = [];
        bufferStart = null;
        bufferSource = null;
        bufferLastTimestampMs = null;
        result.push(entry);
        continue;
      }

      if (!bufferStart) {
        bufferStart = entry;
        bufferSource = entry.source || null;
        bufferLastTimestampMs = Date.parse(entry.timestamp);
      }
      if (bufferSource === entry.source) {
        const nextTimestampMs = Date.parse(entry.timestamp);
        const hasValidTimes =
          Number.isFinite(bufferLastTimestampMs) && Number.isFinite(nextTimestampMs);
        const timeGapMs = hasValidTimes && bufferLastTimestampMs !== null
          ? Math.abs(nextTimestampMs - bufferLastTimestampMs)
          : 0;

        const isContinuousStream = bufferSource === 'sdk' || bufferSource === 'normalized';

        if (!isContinuousStream && hasValidTimes && timeGapMs > 2000) {
          if (textBuffer.length > 0 && bufferStart) {
            result.push({
              ...bufferStart,
              content: joinFragments(textBuffer),
            });
          }
          textBuffer = [entry.content];
          bufferStart = entry;
          bufferSource = entry.source || null;
          bufferLastTimestampMs = nextTimestampMs;
        } else {
          if (isContinuousStream) {
            if (textBuffer.length === 0) {
              textBuffer.push(entry.content);
            } else {
              const lastIndex = textBuffer.length - 1;
              const merged = mergeContinuousStreamFragment(textBuffer[lastIndex], entry.content);
              if (merged.shouldFlush) {
                if (bufferStart) {
                  result.push({
                    ...bufferStart,
                    content: joinFragments(textBuffer),
                  });
                }
                textBuffer = [entry.content];
                bufferStart = entry;
                bufferSource = entry.source || null;
              } else {
                textBuffer[lastIndex] = merged.content;
              }
            }
          } else {
            textBuffer.push(entry.content);
          }
          bufferLastTimestampMs = nextTimestampMs;
        }
      } else {
        if (textBuffer.length > 0 && bufferStart) {
          result.push({
            ...bufferStart,
            content: joinFragments(textBuffer),
          });
        }
        textBuffer = [entry.content];
        bufferStart = entry;
        bufferSource = entry.source || null;
        bufferLastTimestampMs = Date.parse(entry.timestamp);
      }
    } else {
      if (textBuffer.length > 0 && bufferStart) {
        result.push({
          ...bufferStart,
          content: joinFragments(textBuffer),
        });
      }
      textBuffer = [];
      bufferStart = null;
      bufferSource = null;
      bufferLastTimestampMs = null;
      result.push(entry);
    }
  }

  if (textBuffer.length > 0 && bufferStart) {
    result.push({
      ...bufferStart,
      content: joinFragments(textBuffer),
    });
  }

  return result;
}
