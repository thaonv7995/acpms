import { useMemo } from 'react';

interface RawLogTextProps {
  text: string;
}

const URL_REGEX = /(https?:\/\/[^\s]+)/g;

/**
 * Render plain text with automatic URL linkification.
 * Converts URLs into clickable <a> tags while preserving formatting.
 */
export function RawLogText({ text }: RawLogTextProps) {
  const parts = useMemo(() => {
    return text.split(URL_REGEX).map((part, idx) => {
      // Even indices are text, odd indices are URLs
      if (idx % 2 === 1) {
        return (
          <a
            key={idx}
            href={part}
            target="_blank"
            rel="noopener noreferrer"
            className="text-primary underline hover:text-primary/80 transition-colors"
          >
            {part}
          </a>
        );
      }

      // Text part - preserve whitespace and line breaks
      return <span key={idx}>{part}</span>;
    });
  }, [text]);

  return <>{parts}</>;
}
