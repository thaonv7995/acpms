import { RawLogText } from '../RawLogText';

interface FileReadEntryProps {
  path: string;
  content: string;
}

/**
 * Display file read operation details.
 */
export function FileReadEntry({ path, content }: FileReadEntryProps) {
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
