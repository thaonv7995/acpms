import type { AgentLogEntry } from '../../api/agentLogs';

interface LogEntryProps {
    log: AgentLogEntry;
}

export function LogEntry({ log }: LogEntryProps) {
    const getLogTypeStyle = () => {
        switch (log.log_type) {
            case 'stdout':
                return {
                    badge: 'text-green-600 dark:text-green-400 bg-green-100 dark:bg-green-900/20',
                    label: 'stdout',
                    icon: 'terminal',
                    border: 'border-transparent hover:border-green-500',
                };
            case 'stderr':
                return {
                    badge: 'text-red-600 dark:text-red-400 bg-red-100 dark:bg-red-900/20',
                    label: 'stderr',
                    icon: 'error',
                    border: 'border-transparent hover:border-red-500',
                };
            case 'system':
                return {
                    badge: 'text-blue-600 dark:text-blue-400 bg-blue-100 dark:bg-blue-900/20',
                    label: 'system',
                    icon: 'info',
                    border: 'border-transparent hover:border-blue-500',
                };
            default:
                return {
                    badge: 'text-slate-600 dark:text-slate-400 bg-slate-100 dark:bg-slate-900/20',
                    label: 'log',
                    icon: 'description',
                    border: 'border-transparent',
                };
        }
    };

    const formatTimestamp = (dateStr: string) => {
        try {
            const date = new Date(dateStr);
            return date.toLocaleTimeString('en-US', {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                hour12: false,
            });
        } catch {
            return dateStr;
        }
    };

    const style = getLogTypeStyle();

    return (
        <div className={`flex gap-4 px-4 py-2 hover:bg-slate-50 dark:hover:bg-[#15181c] transition-colors border-l-2 ${style.border}`}>
            <div className="w-20 text-xs text-slate-400 dark:text-gray-500 text-right shrink-0 select-none pt-0.5 font-mono">
                {formatTimestamp(log.created_at)}
            </div>
            <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                    <span className={`text-[10px] font-bold px-1.5 py-0.5 rounded uppercase tracking-wider ${style.badge}`}>
                        {style.label}
                    </span>
                    <span className="text-xs text-slate-500 dark:text-gray-400 truncate" title={log.task_title}>
                        {log.task_title}
                    </span>
                    <span className="text-[10px] text-slate-400 dark:text-gray-500">•</span>
                    <span className="text-[10px] text-slate-400 dark:text-gray-500 truncate" title={log.project_name}>
                        {log.project_name}
                    </span>
                </div>
                <pre className={`text-sm whitespace-pre-wrap break-words font-mono ${
                    log.log_type === 'stderr'
                        ? 'text-red-600 dark:text-red-400'
                        : 'text-slate-700 dark:text-gray-300'
                }`}>
                    {log.content}
                </pre>
            </div>
        </div>
    );
}
