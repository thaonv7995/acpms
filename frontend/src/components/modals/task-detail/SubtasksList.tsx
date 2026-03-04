interface SubtasksListProps {
    subtasks: Array<{ id: string; label: string; completed: boolean }>;
    onToggleSubtask?: (id: string) => void;
}

export function SubtasksList({ subtasks, onToggleSubtask }: SubtasksListProps) {
    return (
        <div>
            <h3 className="text-xs font-bold text-slate-500 uppercase mb-3 flex items-center gap-2">
                <span className="material-symbols-outlined text-[16px]">checklist</span>
                Subtasks
            </h3>
            <div className="space-y-2">
                {subtasks.map((subtask) => (
                    <label
                        key={subtask.id}
                        className="flex items-center gap-3 p-3 rounded-lg border border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-800/50 cursor-pointer transition-colors"
                    >
                        <input
                            type="checkbox"
                            checked={subtask.completed}
                            onChange={() => onToggleSubtask?.(subtask.id)}
                            className="rounded border-slate-300 dark:border-slate-600 text-primary focus:ring-primary bg-transparent"
                        />
                        <span className="text-sm text-slate-700 dark:text-slate-300">{subtask.label}</span>
                    </label>
                ))}
            </div>
        </div>
    );
}
