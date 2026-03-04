import { RawLogText } from '../RawLogText';

interface SearchEntryProps {
  query: string;
  results: string;
}

/**
 * Display search operation details.
 */
export function SearchEntry({ query, results }: SearchEntryProps) {
  return (
    <div className="space-y-2">
      <div className="text-sm text-muted-foreground">
        Query: <span className="font-mono">{query}</span>
      </div>
      <div className="text-xs text-muted-foreground max-h-64 overflow-y-auto">
        <RawLogText text={results} />
      </div>
    </div>
  );
}
