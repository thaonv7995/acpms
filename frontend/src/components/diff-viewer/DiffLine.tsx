/**
 * DiffLine - Individual diff line with syntax highlighting
 *
 * Features:
 * - Syntax highlighting using Prism.js
 * - Line number display
 * - Add/delete/normal line styling
 * - Hover state for line selection
 */

import { memo, useMemo } from 'react';
import { clsx } from 'clsx';
import Prism from 'prismjs';
import 'prismjs/components/prism-typescript';
import 'prismjs/components/prism-javascript';
import 'prismjs/components/prism-jsx';
import 'prismjs/components/prism-tsx';
import 'prismjs/components/prism-css';
import 'prismjs/components/prism-scss';
import 'prismjs/components/prism-json';
import 'prismjs/components/prism-yaml';
import 'prismjs/components/prism-markdown';
import 'prismjs/components/prism-bash';
import 'prismjs/components/prism-python';
import 'prismjs/components/prism-rust';
import 'prismjs/components/prism-go';
import 'prismjs/components/prism-sql';
import 'prismjs/components/prism-toml';
import type { DiffLineType } from './types';

interface DiffLineProps {
  type: DiffLineType;
  content: string;
  oldLine?: number;
  newLine?: number;
  language?: string;
  showOldLine?: boolean;
  showNewLine?: boolean;
  onClick?: () => void;
}

// Map internal language names to Prism grammar names
const PRISM_LANG_MAP: Record<string, string> = {
  typescript: 'typescript',
  javascript: 'javascript',
  jsx: 'jsx',
  tsx: 'tsx',
  css: 'css',
  scss: 'scss',
  json: 'json',
  yaml: 'yaml',
  markdown: 'markdown',
  bash: 'bash',
  python: 'python',
  rust: 'rust',
  go: 'go',
  sql: 'sql',
  toml: 'toml',
  plaintext: 'plaintext',
};

function highlightCode(code: string, language: string): string {
  const prismLang = PRISM_LANG_MAP[language] || 'plaintext';
  const grammar = Prism.languages[prismLang];

  if (!grammar) {
    return escapeHtml(code);
  }

  try {
    return Prism.highlight(code, grammar, prismLang);
  } catch {
    return escapeHtml(code);
  }
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

export const DiffLine = memo(function DiffLine({
  type,
  content,
  oldLine,
  newLine,
  language = 'plaintext',
  showOldLine = true,
  showNewLine = true,
  onClick,
}: DiffLineProps) {
  const highlightedContent = useMemo(() => highlightCode(content, language), [content, language]);

  const bgClass = {
    add: 'bg-emerald-500/10',
    del: 'bg-red-500/10',
    normal: '',
  }[type];

  const lineNumClass = {
    add: 'text-emerald-500',
    del: 'text-red-500',
    normal: 'text-muted-foreground',
  }[type];

  const prefixChar = {
    add: '+',
    del: '-',
    normal: ' ',
  }[type];

  const prefixClass = {
    add: 'text-emerald-500',
    del: 'text-red-500',
    normal: 'text-muted-foreground',
  }[type];

  return (
    <div
      className={clsx(
        'flex items-stretch font-mono text-[13px] leading-[1.5] min-h-[22px]',
        bgClass,
        onClick && 'cursor-pointer hover:bg-muted/40'
      )}
      onClick={onClick}
    >
      {/* Old line number */}
      {showOldLine && (
        <span
          className={clsx(
            'w-[50px] px-2 text-right select-none shrink-0 border-r border-border',
            lineNumClass
          )}
        >
          {type !== 'add' ? oldLine : ''}
        </span>
      )}

      {/* New line number */}
      {showNewLine && (
        <span
          className={clsx(
            'w-[50px] px-2 text-right select-none shrink-0 border-r border-border',
            lineNumClass
          )}
        >
          {type !== 'del' ? newLine : ''}
        </span>
      )}

      {/* Prefix (+/-/space) */}
      <span className={clsx('w-[20px] text-center select-none shrink-0', prefixClass)}>{prefixChar}</span>

      {/* Content with syntax highlighting */}
      <span
        className="flex-1 px-2 whitespace-pre overflow-x-auto"
        dangerouslySetInnerHTML={{ __html: highlightedContent || '&nbsp;' }}
      />
    </div>
  );
});

// Unified view line - single line number column
export const UnifiedDiffLine = memo(function UnifiedDiffLine({
  type,
  content,
  lineNumber,
  language = 'plaintext',
  onClick,
}: {
  type: DiffLineType;
  content: string;
  lineNumber?: number;
  language?: string;
  onClick?: () => void;
}) {
  const highlightedContent = useMemo(() => highlightCode(content, language), [content, language]);

  const bgClass = {
    add: 'bg-emerald-500/10',
    del: 'bg-red-500/10',
    normal: '',
  }[type];

  const lineNumClass = {
    add: 'text-emerald-500',
    del: 'text-red-500',
    normal: 'text-muted-foreground',
  }[type];

  const prefixChar = {
    add: '+',
    del: '-',
    normal: ' ',
  }[type];

  const prefixClass = {
    add: 'text-emerald-500',
    del: 'text-red-500',
    normal: 'text-muted-foreground',
  }[type];

  return (
    <div
      className={clsx(
        'flex items-stretch font-mono text-[13px] leading-[1.5] min-h-[22px]',
        bgClass,
        onClick && 'cursor-pointer hover:bg-muted/40'
      )}
      onClick={onClick}
    >
      {/* Line number */}
      <span
        className={clsx(
          'w-[60px] px-2 text-right select-none shrink-0 border-r border-border',
          lineNumClass
        )}
      >
        {lineNumber || ''}
      </span>

      {/* Prefix (+/-/space) */}
      <span className={clsx('w-[20px] text-center select-none shrink-0', prefixClass)}>{prefixChar}</span>

      {/* Content with syntax highlighting */}
      <span
        className="flex-1 px-2 whitespace-pre overflow-x-auto"
        dangerouslySetInnerHTML={{ __html: highlightedContent || '&nbsp;' }}
      />
    </div>
  );
});
