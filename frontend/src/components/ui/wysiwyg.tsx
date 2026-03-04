import { cn } from '@/lib/utils';

interface WYSIWYGEditorProps {
  value: string;
  disabled?: boolean;
  className?: string;
}

/**
 * Simple WYSIWYGEditor component - displays markdown text
 * For now, this is a simple renderer. Full Lexical editor can be added later.
 */
export default function WYSIWYGEditor({
  value,
  disabled = false,
  className,
}: WYSIWYGEditorProps) {
  // Simple markdown rendering - convert markdown to HTML
  const renderMarkdown = (text: string): string => {
    // Basic markdown parsing
    let html = text
      // Headers
      .replace(/^# (.*$)/gim, '<h1>$1</h1>')
      .replace(/^## (.*$)/gim, '<h2>$1</h2>')
      .replace(/^### (.*$)/gim, '<h3>$1</h3>')
      // Bold
      .replace(/\*\*(.*?)\*\*/gim, '<strong>$1</strong>')
      // Italic
      .replace(/\*(.*?)\*/gim, '<em>$1</em>')
      // Code blocks
      .replace(/```([\s\S]*?)```/gim, '<pre><code>$1</code></pre>')
      // Inline code
      .replace(/`([^`]+)`/gim, '<code>$1</code>')
      // Line breaks
      .replace(/\n/gim, '<br />');

    return html;
  };

  if (disabled) {
    return (
      <div
        className={cn('prose prose-sm max-w-none text-sm', className)}
        dangerouslySetInnerHTML={{ __html: renderMarkdown(value) }}
      />
    );
  }

  return (
    <div className={cn('prose prose-sm max-w-none text-sm', className)}>
      <div dangerouslySetInnerHTML={{ __html: renderMarkdown(value) }} />
    </div>
  );
}
