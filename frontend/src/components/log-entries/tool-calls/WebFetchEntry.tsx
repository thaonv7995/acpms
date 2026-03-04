import { RawLogText } from '../RawLogText';

interface WebFetchEntryProps {
  url: string;
  response: string;
}

/**
 * Display web fetch operation details.
 */
export function WebFetchEntry({ url, response }: WebFetchEntryProps) {
  return (
    <div className="space-y-2">
      <div className="text-sm text-muted-foreground">
        URL:{' '}
        <a
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          className="text-primary underline hover:text-primary/80"
        >
          {url}
        </a>
      </div>
      <div className="text-xs text-muted-foreground max-h-64 overflow-y-auto">
        <RawLogText text={response} />
      </div>
    </div>
  );
}
