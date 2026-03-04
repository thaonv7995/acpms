import { RawLogText } from '../RawLogText';

interface TodoManagementEntryProps {
  action: string;
  data: string;
}

/**
 * Display todo management operation details.
 */
export function TodoManagementEntry({ action, data }: TodoManagementEntryProps) {
  return (
    <div className="space-y-2">
      <div className="text-sm text-muted-foreground">
        Action: <span className="font-mono">{action}</span>
      </div>
      <div className="text-xs text-muted-foreground">
        <RawLogText text={data} />
      </div>
    </div>
  );
}
