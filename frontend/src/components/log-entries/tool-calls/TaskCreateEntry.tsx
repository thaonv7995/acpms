import { RawLogText } from '../RawLogText';

interface TaskCreateEntryProps {
  taskData: string;
}

/**
 * Display task creation details.
 */
export function TaskCreateEntry({ taskData }: TaskCreateEntryProps) {
  return (
    <div className="space-y-2">
      <div className="text-xs text-muted-foreground">
        <RawLogText text={taskData} />
      </div>
    </div>
  );
}
