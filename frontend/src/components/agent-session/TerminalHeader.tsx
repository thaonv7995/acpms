/**
 * TerminalHeader - macOS-style terminal window header
 * Shows traffic light buttons and running status
 */

import { memo } from 'react';

interface TerminalHeaderProps {
  isRunning: boolean;
}

export const TerminalHeader = memo(function TerminalHeader({ isRunning }: TerminalHeaderProps) {
  return (
    <div className="flex items-center gap-2 px-4 py-2 bg-slate-800 dark:bg-slate-800/80 border-b border-slate-700">
      <div className="flex gap-1.5">
        <div className="size-3 rounded-full bg-red-500/80" />
        <div className="size-3 rounded-full bg-yellow-500/80" />
        <div className="size-3 rounded-full bg-green-500/80" />
      </div>
      <span className="text-xs text-slate-400 font-mono ml-2">Agent Terminal</span>
      {isRunning && (
        <span className="ml-auto flex items-center gap-1.5 text-xs text-blue-400">
          <span className="size-1.5 bg-blue-400 rounded-full animate-pulse" />
          Running
        </span>
      )}
    </div>
  );
});
