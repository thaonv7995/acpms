// LogsTab Component for ProjectDetail
import type { ProjectAgentLog } from '../../types/project';

interface LogsTabProps {
    logs: ProjectAgentLog[];
    onFilter?: (filter: string) => void;
    onDownload?: () => void;
}

const levelStyles: Record<ProjectAgentLog['level'], string> = {
    info: 'text-blue-400',
    debug: 'text-slate-500',
    warn: 'text-amber-500',
    error: 'text-red-500',
};

export function LogsTab({ logs, onFilter, onDownload }: LogsTabProps) {
    return (
        <div className="flex flex-col h-[600px] bg-slate-50 dark:bg-[#0d1117] rounded-xl border border-slate-200 dark:border-slate-700 overflow-hidden">
            {/* Toolbar */}
            <div className="px-4 py-3 bg-white dark:bg-[#161b22] border-b border-slate-200 dark:border-slate-700 flex justify-between items-center">
                <div className="flex gap-2">
                    <div className="relative">
                        <span className="absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400 material-symbols-outlined text-[18px]">search</span>
                        <input
                            type="text"
                            placeholder="Filter logs..."
                            onChange={(e) => onFilter?.(e.target.value)}
                            className="pl-8 pr-3 py-1.5 bg-slate-100 dark:bg-[#0d1117] border-none rounded-md text-sm text-slate-900 dark:text-white focus:ring-1 focus:ring-primary w-64"
                        />
                    </div>
                    <button className="px-3 py-1.5 bg-slate-100 dark:bg-[#0d1117] hover:bg-slate-200 dark:hover:bg-[#21262d] rounded-md text-xs font-medium text-slate-600 dark:text-slate-300 border border-transparent hover:border-slate-300 dark:hover:border-slate-600 transition-colors">
                        Errors Only
                    </button>
                </div>
                <div className="flex items-center gap-3">
                    <span className="flex items-center gap-2 text-xs text-slate-500 dark:text-slate-400">
                        <span className="size-2 rounded-full bg-green-500 animate-pulse"></span>
                        Live Streaming
                    </span>
                    <button onClick={onDownload} className="text-slate-400 hover:text-primary transition-colors">
                        <span className="material-symbols-outlined text-[20px]">download</span>
                    </button>
                </div>
            </div>

            {/* Log Content */}
            <div className="flex-1 overflow-y-auto p-4 font-mono text-xs">
                {logs.map((log) => (
                    <div key={log.id} className="flex gap-4 py-1.5 hover:bg-slate-100 dark:hover:bg-white/5 px-2 rounded -mx-2 transition-colors group">
                        <span className="text-slate-400 dark:text-slate-600 shrink-0 select-none">{log.timestamp}</span>
                        <span className={`shrink-0 w-24 font-bold ${log.agentColor}`}>{log.agentName}</span>
                        <span className={`shrink-0 w-12 font-bold uppercase ${levelStyles[log.level]}`}>{log.level}</span>
                        <span className={`text-slate-700 dark:text-slate-300 break-all ${log.level === 'error' ? 'text-red-600 dark:text-red-400' : ''}`}>
                            {log.message}
                        </span>
                    </div>
                ))}

                {logs.length === 0 && (
                    <div className="text-center text-slate-400 py-8">
                        No logs available
                    </div>
                )}
            </div>
        </div>
    );
}
