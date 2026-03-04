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
          textBuffer.push(entry.content);
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
