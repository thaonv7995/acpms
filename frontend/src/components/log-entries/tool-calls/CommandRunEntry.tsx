import { RawLogText } from '../RawLogText';
import { formatShellCommandForDisplay } from '@/lib/commandDisplay';
import { formatExitCode } from '@/utils/formatters';
import type { CommandRunResult } from '@/bindings/CommandRunResult';

interface CommandRunEntryProps {
  command: string;
  output: string;
  result: CommandRunResult | null;
}

/**
 * Display command execution details.
 */
export function CommandRunEntry({ command, output, result }: CommandRunEntryProps) {
  const exitCode = result?.exit_status?.type === 'exit_code' ? result.exit_status.code : undefined;
  const displayCommand = formatShellCommandForDisplay(command);

  return (
    <div className="space-y-2">
      <div className="text-sm font-mono text-muted-foreground">
        Command: {displayCommand}
      </div>
      {exitCode !== undefined && (
        <div className="text-xs">
          Exit Code: <span className={exitCode === 0 ? 'text-green-500' : 'text-red-500'}>
            {formatExitCode(exitCode)}
          </span>
        </div>
      )}
      <div className="text-xs text-muted-foreground max-h-64 overflow-y-auto font-mono bg-muted/50 p-2 rounded">
        <RawLogText text={output} />
      </div>
    </div>
  );
}
