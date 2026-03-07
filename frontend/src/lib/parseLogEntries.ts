/**
 * R5: Single transform layer - Raw logs → TimelineEntry[].
 * Consolidates normalizeLogToEntry + combineTextFragments into one parse flow.
 */
import type { TimelineEntry } from '@/types/timeline-log';
import {
  formatBreakdownTaskContent,
  normalizeLogToEntry,
  type AgentLogLike,
} from './normalizeLogToEntry';
import { combineTextFragments } from './timeline-fragments';

const COMMON_SHORT_WORDS = new Set([
  'a',
  'all',
  'an',
  'and',
  'are',
  'at',
  'be',
  'but',
  'by',
  'for',
  'from',
  'git',
  'in',
  'into',
  'is',
  'it',
  'no',
  'not',
  'of',
  'on',
  'or',
  'the',
  'to',
  'url',
  'web',
  'with',
]);

const COMPOUND_PREFIXES = new Set(['cloud', 'git', 'pre', 'type']);

function joinFragmentedAssistantText(left: string, right: string): string {
  const prev = left.trimEnd();
  const next = right.trimStart();
  if (!prev) return next;
  if (!next) return prev;

  if (/^#{1,6}$/.test(prev)) {
    return `${prev} ${next}`;
  }

  if (prev === '-' || prev === '*' || prev === '•') {
    return `- ${next}`;
  }

  const last = prev.slice(-1);
  if (
    /^[.,!?;:%)\]}]/.test(next) ||
    /^['’](s|t|re|ve|ll|d|m)\b/i.test(next) ||
    /^[`_*]/.test(next) ||
    last === '/' ||
    last === '-' ||
    last === '`' ||
    /\*\*$/.test(prev) ||
    next.startsWith('-')
  ) {
    return `${prev}${next}`;
  }

  const prevWord = prev.match(/([A-Za-z]{1,12})$/)?.[1];
  const nextWord = next.match(/^([A-Za-z]{1,16})/)?.[1];
  if (prevWord && nextWord) {
    const prevLower = prevWord.toLowerCase();
    if (
      prevWord.length <= 4 &&
      /^[a-z]/.test(nextWord) &&
      !COMMON_SHORT_WORDS.has(prevLower)
    ) {
      return `${prev}${next}`;
    }

    if (
      COMPOUND_PREFIXES.has(prevLower) &&
      /^[A-Z]/.test(nextWord)
    ) {
      return `${prev}${next}`;
    }
  }

  return `${prev} ${next}`;
}

function isHeadingLikeFragment(text: string): boolean {
  const compact = text.trim();
  return (
    compact.length > 0 &&
    compact.length <= 24 &&
    !/[.:/]/.test(compact) &&
    /^[A-Z&+][A-Za-z0-9&+ -]*$/.test(compact)
  );
}

function isShortHeadingFragment(text: string): boolean {
  const compact = text.trim();
  return compact.length <= 18 || compact.split(/\s+/).length <= 2;
}

function isFragmentedMarkdownBoundary(text: string): boolean {
  const compact = text.trim();
  return (
    compact === '---' ||
    compact === '•' ||
    compact === '-' ||
    /^#{1,6}$/.test(compact) ||
    /^#{1,6}\s+/.test(compact) ||
    /^-\s+/.test(compact) ||
    /^\d+\.\s+/.test(compact)
  );
}

function isMarkdownHeading(line: string): boolean {
  return /^#{1,6}\s+/.test(line.trim());
}

function isMarkdownListItem(line: string): boolean {
  return /^-\s+/.test(line.trim()) || /^\d+\.\s+/.test(line.trim());
}

function applyFragmentReplacements(content: string): string {
  let fixed = content
    .replace(/\bType Script\b/g, 'TypeScript')
    .replace(/\bGit Lab\b/g, 'GitLab')
    .replace(/\bCloud flare\b/g, 'Cloudflare')
    .replace(/\bPre flight\b/g, 'Preflight')
    .replace(/\bGITLAB _PA T\b/g, 'GITLAB_PAT')
    .replace(/\bGITLAB _URL\b/g, 'GITLAB_URL')
    .replace(/\bREPO _URL\b/g, 'REPO_URL')
    .replace(/\bDEPLOY _PRECHECK\b/g, 'DEPLOY_PRECHECK')
    .replace(/\bCF _ACCOUNT_ID\b/g, 'CF_ACCOUNT_ID')
    .replace(/\bCF _API_TOKEN\b/g, 'CF_API_TOKEN')
    .replace(/\bCLOUDFLARE_ACCOUNT_ ID\b/g, 'CLOUDFLARE_ACCOUNT_ID')
    .replace(/\bCLOUDFLARE _API_TOKEN\b/g, 'CLOUDFLARE_API_TOKEN')
    .replace(/\. acpms/g, '.acpms')
    .replace(/\/ refs _ manifest \.json/g, '/refs_manifest.json');

  fixed = fixed
    .replace(/\b([A-Z]{2,}_[A-Z]{2,})\s+([A-Z]{1,4})\b/g, '$1$2')
    .replace(/\b([A-Z]{2,})\s+_([A-Z]{2,})\b/g, '$1_$2')
    .replace(/\bsk ipped\b/gi, 'skipped')
    .replace(/\bsk ipped_/gi, 'skipped_')
    .replace(/\brefsmanifest\b/gi, 'refs manifest')
    .replace(/\brefsdirectory\b/gi, 'refs directory')
    .replace(/\bcloud flare_not _configured\b/gi, 'cloudflare_not_configured')
    .replace(/\bAuto-deploy\b/g, 'Auto-deploy')
    .replace(/\bC TA\b/g, 'CTA')
    .replace(/\s-\s*no\b/gi, ' - no')
    .replace(/(^|\n)\s*-\s+no\b/g, '$1- No')
    .replace(/\(\s+HTTP\s+(\d+)\)/g, '(HTTP $1)')
    .replace(/https?:\/\/[A-Za-z0-9./:_-]+(?:\s+[A-Za-z0-9./:_-]+)+/g, (match) =>
      match.replace(/\s+/g, '')
    )
    .replace(/`([^`]+)`/g, (_, inner: string) => `\`${inner.replace(/\s+/g, '')}\``);

  return fixed;
}

function rebuildFragmentedMarkdown(rawLines: string[]): string {
  const logicalLines: string[] = [];
  let current = '';

  const flushCurrent = () => {
    const trimmed = current.trim();
    if (trimmed) {
      logicalLines.push(trimmed === '•' ? '-' : trimmed);
    }
    current = '';
  };

  for (const rawLine of rawLines) {
    const token = rawLine.trim();
    if (!token) {
      flushCurrent();
      continue;
    }

    if (token === '•') {
      flushCurrent();
      current = '-';
      continue;
    }

    if (token === '---') {
      flushCurrent();
      logicalLines.push('---');
      continue;
    }

    if (isFragmentedMarkdownBoundary(token)) {
      if (current && !/^#{1,6}$/.test(current.trim()) && current.trim() !== '-') {
        flushCurrent();
      }

      if (/^#{1,6}$/.test(token) || token === '-') {
        flushCurrent();
        current = token;
      } else {
        flushCurrent();
        current = token;
      }
      continue;
    }

    current = current ? joinFragmentedAssistantText(current, token) : token;
  }

  flushCurrent();

  const mergedHeadings: string[] = [];
  for (const line of logicalLines) {
    const previous = mergedHeadings[mergedHeadings.length - 1];
    if (
      previous &&
      !isFragmentedMarkdownBoundary(line) &&
      isHeadingLikeFragment(line) &&
      (
        isMarkdownHeading(previous) ||
        (isHeadingLikeFragment(previous) &&
          isShortHeadingFragment(previous) &&
          isShortHeadingFragment(line))
      )
    ) {
      mergedHeadings[mergedHeadings.length - 1] = joinFragmentedAssistantText(previous, line);
      continue;
    }
    mergedHeadings.push(line);
  }

  const formatted: string[] = [];
  for (const line of mergedHeadings) {
    const trimmed = line.trim();
    const isSectionish =
      trimmed === '---' || isMarkdownHeading(trimmed) || isMarkdownListItem(trimmed);

    if (isSectionish && formatted.length > 0 && formatted[formatted.length - 1] !== '') {
      formatted.push('');
    }

    formatted.push(trimmed);

    if ((trimmed === '---' || isMarkdownHeading(trimmed)) && formatted[formatted.length - 1] !== '') {
      formatted.push('');
    }
  }

  return applyFragmentReplacements(
    formatted
      .join('\n')
      .replace(/\n{3,}/g, '\n\n')
      .trim()
  );
}

export function repairFragmentedAssistantLayout(content: string): string {
  if (!content || !content.includes('\n')) {
    return content;
  }

  const normalized = content.replace(/\r\n/g, '\n');
  const rawLines = normalized.split('\n');
  const nonEmpty = rawLines.filter((line) => line.trim().length > 0);
  if (nonEmpty.length < 6) {
    return normalized;
  }

  const shortLineRatio =
    nonEmpty.filter((line) => line.trim().length <= 18).length / nonEmpty.length;
  if (shortLineRatio < 0.55) {
    return normalized;
  }

  const hasMarkdownStructure = nonEmpty.some((line) => isFragmentedMarkdownBoundary(line));
  if (hasMarkdownStructure) {
    return rebuildFragmentedMarkdown(rawLines);
  }

  const paragraphs: string[] = [];
  let current = '';

  for (const rawLine of rawLines) {
    const line = rawLine.trim();
    if (!line) {
      if (current.trim()) {
        paragraphs.push(current.trim());
        current = '';
      }
      continue;
    }

    current = current
      ? joinFragmentedAssistantText(current, line)
      : line;
  }

  if (current.trim()) {
    paragraphs.push(current.trim());
  }

  const mergedParagraphs: string[] = [];
  for (const paragraph of paragraphs) {
    const last = mergedParagraphs[mergedParagraphs.length - 1];
    if (
      last &&
      isHeadingLikeFragment(last) &&
      isHeadingLikeFragment(paragraph) &&
      isShortHeadingFragment(last) &&
      isShortHeadingFragment(paragraph)
    ) {
      mergedParagraphs[mergedParagraphs.length - 1] = `${last} ${paragraph}`;
      continue;
    }
    mergedParagraphs.push(paragraph);
  }

  return applyFragmentReplacements(mergedParagraphs.join('\n\n'));
}

/**
 * Parse raw agent logs into timeline entries.
 * Flow: normalize (per log) → flatten → combine text fragments.
 */
export function parseLogEntries(rawLogs: AgentLogLike[]): TimelineEntry[] {
  const entries = rawLogs.flatMap((log, index) => normalizeLogToEntry(log, index));
  const combined = combineTextFragments(entries);
  return combined.map((entry) => {
    if (entry.type !== 'assistant_message' && entry.type !== 'thinking') {
      return entry;
    }
    const repairedContent = repairFragmentedAssistantLayout(entry.content);
    if (entry.type === 'thinking') {
      return {
        ...entry,
        content: repairedContent,
      };
    }
    const formattedBreakdown = formatBreakdownTaskContent(repairedContent);
    if (!formattedBreakdown) {
      return {
        ...entry,
        content: repairedContent,
      };
    }
    return {
      ...entry,
      content: formattedBreakdown,
    };
  });
}
