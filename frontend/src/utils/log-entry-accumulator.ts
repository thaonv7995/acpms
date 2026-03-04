// log-entry-accumulator.ts - Accumulates consecutive log entries into message blocks
import type { PatchTypeWithKey } from '../hooks/useConversationHistory';

/**
 * Accumulate consecutive STDOUT/STDERR entries into message blocks.
 * This prevents fragmented display of streaming text deltas.
 *
 * Example: ["I'll", " help", " you"] -> ["I'll help you"]
 */
export function accumulateEntries(entries: PatchTypeWithKey[]): PatchTypeWithKey[] {
  if (!entries.length) return [];

  const result: PatchTypeWithKey[] = [];
  let currentBlock: {
    type: string;
    content: string[];
    timestamp: string;
    patchKey: string;
    executionProcessId: string;
  } | null = null;

  const flushBlock = () => {
    if (currentBlock) {
      result.push({
        id: currentBlock.patchKey,
        type: currentBlock.type,
        content: currentBlock.content.join(''),
        timestamp: currentBlock.timestamp,
        patchKey: currentBlock.patchKey,
        executionProcessId: currentBlock.executionProcessId,
      } as PatchTypeWithKey);
      currentBlock = null;
    }
  };

  for (const entry of entries) {
    // Handle NORMALIZED_ENTRY types - always add as-is
    if (entry.type === 'NORMALIZED_ENTRY') {
      flushBlock();
      result.push(entry);
      continue;
    }

    // Handle "normalized" log type from backend - always add as-is (don't accumulate)
    if (entry.type === 'normalized') {
      flushBlock();
      result.push(entry);
      continue;
    }

    // Handle STDOUT entries - accumulate into blocks
    if (entry.type === 'STDOUT') {
      const content = typeof entry.content === 'string' ? entry.content : String(entry.content || '');

      if (currentBlock && currentBlock.type === 'STDOUT') {
        currentBlock.content.push(content);
      } else {
        flushBlock();
        currentBlock = {
          type: 'STDOUT',
          content: [content],
          timestamp: entry.timestamp || new Date().toISOString(),
          patchKey: entry.patchKey,
          executionProcessId: entry.executionProcessId,
        };
      }
      continue;
    }

    // Handle STDERR entries - accumulate separately
    if (entry.type === 'STDERR') {
      const content = typeof entry.content === 'string' ? entry.content : String(entry.content || '');

      if (currentBlock && currentBlock.type === 'STDERR') {
        currentBlock.content.push(content);
      } else {
        flushBlock();
        currentBlock = {
          type: 'STDERR',
          content: [content],
          timestamp: entry.timestamp || new Date().toISOString(),
          patchKey: entry.patchKey,
          executionProcessId: entry.executionProcessId,
        };
      }
      continue;
    }

    // Any other type - flush block and add as-is
    flushBlock();
    result.push(entry);
  }

  // Flush final block
  flushBlock();

  return result;
}
