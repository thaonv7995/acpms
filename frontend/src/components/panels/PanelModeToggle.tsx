import { Terminal, Eye, GitCompare } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { LayoutMode } from '../layout/TasksLayout';

interface PanelModeToggleProps {
  mode: LayoutMode;
  onModeChange: (mode: LayoutMode) => void;
}

const modes = [
  { value: null, label: 'Attempt', icon: Terminal },
  { value: 'preview' as const, label: 'Preview', icon: Eye },
  { value: 'diffs' as const, label: 'Diffs', icon: GitCompare },
];

export function PanelModeToggle({ mode, onModeChange }: PanelModeToggleProps) {
  return (
    <div className="flex bg-slate-100 dark:bg-slate-800 rounded-lg p-1">
      {modes.map(({ value, label, icon: Icon }) => (
        <button
          key={label}
          onClick={() => onModeChange(value)}
          className={cn(
            'flex items-center gap-1.5 px-3 py-1.5 rounded text-sm font-medium transition-all',
            mode === value
              ? 'bg-white dark:bg-slate-700 shadow text-primary-600 dark:text-primary-400'
              : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
          )}
        >
          <Icon className="h-4 w-4" />
          <span className="hidden sm:inline">{label}</span>
        </button>
      ))}
    </div>
  );
}
