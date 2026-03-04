import { RawLogText } from '../RawLogText';

interface FileEditEntryProps {
  path: string;
  content: string;
}

/**
 * Display file edit/write operation details.
 */
export function FileEditEntry({ path, content }: FileEditEntryProps) {
  return (
    <div className="space-y-2">
      <div className="text-sm font-mono text-muted-foreground">
        Path: {path}
      </div>
      <div className="text-xs text-muted-foreground max-h-64 overflow-y-auto">
        <RawLogText text={content} />
      </div>
    </div>
  );
}
