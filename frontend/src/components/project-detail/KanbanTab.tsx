// KanbanTab Component for ProjectDetail
import { useNavigate } from 'react-router-dom';
import type { KanbanColumn, KanbanTask } from '../../types/project';
import { LiveAgentActivity } from './LiveAgentActivity';

interface KanbanTabProps {
    columns: KanbanColumn[];
    projectId: string;
    onAddTask?: (columnId: string) => void;
    onTaskClick?: (taskId: string) => void;
}

const typeStyles: Record<KanbanTask['type'], { bg: string; text: string; label: string }> = {
    feature: { bg: 'bg-blue-100 dark:bg-blue-900/30', text: 'text-blue-600 dark:text-blue-400', label: 'Feature' },
    bug: { bg: 'bg-purple-100 dark:bg-purple-900/30', text: 'text-purple-600 dark:text-purple-400', label: 'Bug' },
    hotfix: { bg: 'bg-rose-100 dark:bg-rose-900/30', text: 'text-rose-600 dark:text-rose-400', label: 'Hotfix' },
    refactor: { bg: 'bg-orange-100 dark:bg-orange-900/30', text: 'text-orange-600 dark:text-orange-400', label: 'Refactor' },
    docs: { bg: 'bg-teal-100 dark:bg-teal-900/30', text: 'text-teal-600 dark:text-teal-400', label: 'Docs' },
    test: { bg: 'bg-indigo-100 dark:bg-indigo-900/30', text: 'text-indigo-600 dark:text-indigo-400', label: 'Test' },
    chore: { bg: 'bg-slate-100 dark:bg-slate-900/30', text: 'text-slate-600 dark:text-slate-400', label: 'Chore' },
    spike: { bg: 'bg-amber-100 dark:bg-amber-900/30', text: 'text-amber-600 dark:text-amber-400', label: 'Spike' },
    small_task: { bg: 'bg-slate-100 dark:bg-slate-800', text: 'text-slate-700 dark:text-slate-200', label: 'Small Task' },
    deploy: { bg: 'bg-emerald-100 dark:bg-emerald-900/30', text: 'text-emerald-600 dark:text-emerald-400', label: 'Deploy' },
};

export function KanbanTab({ columns, projectId, onAddTask, onTaskClick }: KanbanTabProps) {
    const navigate = useNavigate();

    const handleTaskClick = (taskId: string) => {
        if (onTaskClick) {
            onTaskClick(taskId);
        } else {
            navigate(`/tasks?taskId=${taskId}`);
        }
    };

    return (
        <div className="grid grid-cols-1 xl:grid-cols-3 gap-6 h-full min-h-[500px]">
            <div className="xl:col-span-2 flex flex-col gap-4">
                <div className="flex gap-4 h-full overflow-x-auto pb-4 no-scrollbar">
                    {columns.map((column) => (
                        <div key={column.id} className="flex-1 min-w-[300px] flex flex-col gap-3">
                            {/* Column Header */}
                            <div className="flex items-center justify-between mb-2">
                                <h3 className={`text-sm font-bold uppercase tracking-wider flex items-center gap-2 ${column.status === 'done' ? 'text-green-600 dark:text-green-400' : 'text-slate-900 dark:text-white'
                                    }`}>
                                    <span className={`size-2 rounded-full ${column.status === 'in_progress' ? 'bg-primary animate-pulse' :
                                        column.status === 'done' ? 'bg-green-500' : 'bg-slate-400'
                                        }`}></span>
                                    {column.title}
                                    <span className={`text-xs px-2 py-0.5 rounded-full ${column.status === 'done'
                                        ? 'bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400'
                                        : 'bg-slate-200 dark:bg-slate-700 text-slate-600 dark:text-slate-300'
                                        }`}>
                                        {column.tasks.length}
                                    </span>
                                </h3>
                                {column.status === 'todo' && (
                                    <button
                                        onClick={() => onAddTask?.(column.id)}
                                        className="text-slate-400 hover:text-primary"
                                    >
                                        <span className="material-symbols-outlined text-lg">add</span>
                                    </button>
                                )}
                            </div>

                            {/* Tasks */}
                            {column.tasks.map((task) => {
                                const typeStyle = typeStyles[task.type];
                                const isDone = task.status === 'done';
                                const isWorking = !!task.agentWorking;

                                return (
                                    <div
                                        key={task.id}
                                        onClick={() => handleTaskClick(task.id)}
                                        className={`bg-white dark:bg-surface-dark border rounded-lg p-4 shadow-sm transition-shadow cursor-pointer group ${isWorking
                                            ? 'border-primary/40 dark:border-primary/40 relative overflow-hidden'
                                            : 'border-slate-200 dark:border-slate-700 hover:shadow-md'
                                            } ${isDone ? 'opacity-75' : ''}`}
                                    >
                                        {/* Progress bar for active tasks */}
                                        {isWorking && (
                                            <div className="absolute top-0 left-0 w-full h-1 bg-slate-200 dark:bg-slate-800">
                                                <div className="h-full bg-primary" style={{ width: `${task.agentWorking!.progress}%` }}></div>
                                            </div>
                                        )}

                                        <div className={`flex justify-between items-start mb-2 ${isWorking ? 'mt-2' : ''}`}>
                                            <span className={`${typeStyle.bg} ${typeStyle.text} text-[10px] font-bold px-2 py-1 rounded uppercase tracking-wider`}>
                                                {isDone ? 'Complete' : typeStyle.label}
                                            </span>
                                            {isDone ? (
                                                <span className="material-symbols-outlined text-green-500 text-lg">check_circle</span>
                                            ) : isWorking ? (
                                                <span className="flex h-2 w-2 relative">
                                                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75"></span>
                                                    <span className="relative inline-flex rounded-full h-2 w-2 bg-primary"></span>
                                                </span>
                                            ) : (
                                                <span className="material-symbols-outlined text-slate-400 text-lg group-hover:text-primary">more_horiz</span>
                                            )}
                                        </div>

                                        <h4 className={`text-slate-900 dark:text-white font-semibold text-sm mb-2 ${isDone ? 'line-through' : ''}`}>
                                            {task.title}
                                        </h4>

                                        {task.description && (
                                            <p className="text-xs text-slate-500 dark:text-[#9cabba] mb-3 line-clamp-2">{task.description}</p>
                                        )}

                                        <div className={`flex items-center justify-between mt-auto ${isWorking ? 'pt-2 border-t border-slate-100 dark:border-slate-800' : ''}`}>
                                            {isWorking ? (
                                                <>
                                                    <div className="flex items-center gap-2">
                                                        <span className="material-symbols-outlined text-primary text-lg">smart_toy</span>
                                                        <span className="text-xs font-medium text-primary">{task.agentWorking!.name} Working...</span>
                                                    </div>
                                                    <span className="text-xs font-mono text-slate-500">{task.agentWorking!.progress}%</span>
                                                </>
                                            ) : task.assignee ? (
                                                <>
                                                    <div className={`size-6 rounded-full ${task.assignee.color} flex items-center justify-center text-[10px] text-white ring-2 ring-white dark:ring-surface-dark`}>
                                                        {task.assignee.initial}
                                                    </div>
                                                    {task.attachments && (
                                                        <div className="flex items-center gap-1 text-slate-400 text-xs">
                                                            <span className="material-symbols-outlined text-sm">attach_file</span>
                                                            {task.attachments}
                                                        </div>
                                                    )}
                                                </>
                                            ) : isDone ? (
                                                <span className="text-xs text-green-500">Completed</span>
                                            ) : null}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    ))}
                </div>
            </div>

            {/* Live Agent Activity */}
            <LiveAgentActivity projectId={projectId} />
        </div>
    );
}
