import { RawLogText } from '../RawLogText';

interface GenericToolEntryProps {
  actionData: string;
}

/**
 * Display generic tool action details (fallback for unknown tool types).
 */
export function GenericToolEntry({ actionData }: GenericToolEntryProps) {
  return (
    <div className="space-y-2">
      <div className="text-xs text-muted-foreground">
        <RawLogText text={actionData} />
      </div>
    </div>
  );
}
