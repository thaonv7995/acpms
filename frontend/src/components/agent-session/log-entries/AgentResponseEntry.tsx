/**
 * AgentResponseEntry - Displays agent text responses
 * Supports markdown rendering for formatted content
 */

import { memo, useMemo } from 'react';
import type { AgentLogEntry } from '../types';

interface AgentResponseEntryProps {
  entry: AgentLogEntry;
}

// Simple markdown-like rendering for common patterns
function renderSimpleMarkdown(text: string): React.ReactNode[] {
  const lines = text.split('\n');
  const elements: React.ReactNode[] = [];

  lines.forEach((line, lineIndex) => {
    const key = `line-${lineIndex}`;

    // Headers
    if (line.startsWith('### ')) {
      elements.push(
        <h4 key={key} className="font-semibold text-slate-200 mt-2 mb-1">
          {line.slice(4)}
        </h4>
      );
      return;
    }
    if (line.startsWith('## ')) {
      elements.push(
        <h3 key={key} className="font-bold text-slate-100 mt-3 mb-1">
          {line.slice(3)}
        </h3>
      );
      return;
    }

    // Bullet points
    if (line.match(/^[\s]*[•\-\*]\s/)) {
      const indent = line.match(/^[\s]*/)?.[0].length || 0;
      const content = line.replace(/^[\s]*[•\-\*]\s/, '');
      elements.push(
        <div key={key} className="flex gap-2" style={{ marginLeft: `${indent * 0.5}rem` }}>
          <span className="text-slate-500">-</span>
          <span>{renderInlineFormatting(content)}</span>
        </div>
      );
      return;
    }

    // Code blocks (simple detection)
    if (line.startsWith('```')) {
      // Skip code fence markers
      return;
    }

    // Empty lines
    if (line.trim() === '') {
      elements.push(<div key={key} className="h-2" />);
      return;
    }

    // Regular text
    elements.push(
      <p key={key} className="leading-relaxed">
        {renderInlineFormatting(line)}
      </p>
    );
  });

  return elements;
}

// Handle inline formatting like **bold**, `code`, etc.
function renderInlineFormatting(text: string): React.ReactNode {
  // Split by code blocks first
  const parts = text.split(/(`[^`]+`)/g);

  return parts.map((part, i) => {
    if (part.startsWith('`') && part.endsWith('`')) {
      return (
        <code
          key={i}
          className="px-1 py-0.5 bg-slate-700/50 rounded text-cyan-300 font-mono text-[0.9em]"
        >
          {part.slice(1, -1)}
        </code>
      );
    }

    // Handle bold
    const boldParts = part.split(/(\*\*[^*]+\*\*)/g);
    return boldParts.map((bp, j) => {
      if (bp.startsWith('**') && bp.endsWith('**')) {
        return (
          <strong key={`${i}-${j}`} className="font-semibold text-slate-200">
            {bp.slice(2, -2)}
          </strong>
        );
      }
      return bp;
    });
  });
}

export const AgentResponseEntry = memo(function AgentResponseEntry({
  entry,
}: AgentResponseEntryProps) {
  const renderedContent = useMemo(() => {
    return renderSimpleMarkdown(entry.content);
  }, [entry.content]);

  return (
    <div className="py-2 text-sm text-slate-300 leading-relaxed space-y-1">
      {renderedContent}
    </div>
  );
});
