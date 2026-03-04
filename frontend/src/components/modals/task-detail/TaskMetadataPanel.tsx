interface TaskMetadataPanelProps {
    assignee: string;
    priority: string;
    estimate: string;
    tags: Array<{ label: string; color: string }>;
}

export function TaskMetadataPanel({ assignee, priority, estimate, tags }: TaskMetadataPanelProps) {
    return (
        <div className="space-y-4">
            <div>
                <label className="text-xs font-bold text-slate-500 uppercase mb-1 block">Assignee</label>
                <div className="flex items-center gap-2 text-sm text-slate-900 dark:text-white font-medium">
                    <div className="size-6 rounded-full bg-slate-300 dark:bg-slate-600"></div>
                    {assignee}
                </div>
            </div>
            <div>
                <label className="text-xs font-bold text-slate-500 uppercase mb-1 block">Priority</label>
                <div className="flex items-center gap-2 text-sm text-slate-900 dark:text-white font-medium">
                    <span className={`material-symbols-outlined text-[18px] ${priority === 'High' || priority === 'Critical' ? 'text-amber-500' : 'text-slate-400'
                        }`}>flag</span>
                    {priority}
                </div>
            </div>
            <div>
                <label className="text-xs font-bold text-slate-500 uppercase mb-1 block">Estimate</label>
                <div className="flex items-center gap-2 text-sm text-slate-900 dark:text-white font-medium">
                    <span className="material-symbols-outlined text-slate-400 text-[18px]">timelapse</span>
                    {estimate}
                </div>
            </div>
            <div>
                <label className="text-xs font-bold text-slate-500 uppercase mb-1 block">Tags</label>
                <div className="flex flex-wrap gap-2">
                    {tags.map((tag, idx) => (
                        <span
                            key={idx}
                            className={`px-2 py-1 rounded ${tag.color} text-[10px] font-medium`}
                        >
                            {tag.label}
                        </span>
                    ))}
                </div>
            </div>
        </div>
    );
}
