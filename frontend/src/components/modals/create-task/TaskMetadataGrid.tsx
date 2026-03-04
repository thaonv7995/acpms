import type { TaskType } from '../../../shared/types';

interface TaskMetadataGridProps {
    type: Exclude<TaskType, 'init'>;
    priority: 'low' | 'medium' | 'high' | 'critical';
    assignee: string;
    sprint: string;
    users: { id: string; name: string }[];
    sprints: { id: string; name: string; status: string }[];
    onTypeChange: (type: Exclude<TaskType, 'init'>) => void;
    onPriorityChange: (priority: 'low' | 'medium' | 'high' | 'critical') => void;
    onAssigneeChange: (assignee: string) => void;
    onSprintChange: (sprint: string) => void;
}

export function TaskMetadataGrid({
    type,
    priority,
    assignee,
    sprint,
    users,
    sprints,
    onTypeChange,
    onPriorityChange,
    onAssigneeChange,
    onSprintChange
}: TaskMetadataGridProps) {
    return (
        <div className="grid grid-cols-2 gap-4">
            <div>
                <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">Type</label>
                <select
                    value={type}
                    onChange={(e) => onTypeChange(e.target.value as TaskMetadataGridProps['type'])}
                    className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                >
                    <option value="feature">Feature</option>
                    <option value="bug">Bug</option>
                    <option value="hotfix">Hotfix</option>
                    <option value="refactor">Refactor</option>
                    <option value="docs">Documentation</option>
                    <option value="test">Test</option>
                    <option value="chore">Chore</option>
                    <option value="spike">Spike (Research)</option>
                    <option value="small_task">Small Task</option>
                    <option value="deploy">Deploy</option>
                </select>
            </div>
            <div>
                <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">Priority</label>
                <select
                    value={priority}
                    onChange={(e) => onPriorityChange(e.target.value as TaskMetadataGridProps['priority'])}
                    className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                >
                    <option value="low">Low</option>
                    <option value="medium">Medium</option>
                    <option value="high">High</option>
                    <option value="critical">Critical</option>
                </select>
            </div>
            <div>
                <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">Assignee</label>
                <div className="relative">
                    <select
                        value={assignee}
                        onChange={(e) => onAssigneeChange(e.target.value)}
                        className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary appearance-none"
                    >
                        <option value="">Unassigned</option>
                        {users.map(user => (
                            <option key={user.id} value={user.id}>{user.name}</option>
                        ))}
                    </select>
                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none material-symbols-outlined text-[18px]">expand_more</span>
                </div>
            </div>
            <div>
                <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">Sprint</label>
                <div className="relative">
                    <select
                        value={sprint}
                        onChange={(e) => onSprintChange(e.target.value)}
                        className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary appearance-none"
                    >
                        <option value="">No Sprint</option>
                        {sprints.map(s => (
                            <option key={s.id} value={s.id}>{s.name}</option>
                        ))}
                    </select>
                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none material-symbols-outlined text-[18px]">expand_more</span>
                </div>
            </div>
        </div>
    );
}
