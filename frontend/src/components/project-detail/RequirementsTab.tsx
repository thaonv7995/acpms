// RequirementsTab Component for ProjectDetail
import { useState } from 'react';
import type { Requirement as ApiRequirement, RequirementStatus } from '../../api/requirements';
import type { Task } from '../../shared/types';

// Extended requirement type that handles both API and legacy mock data
type RequirementDisplay = ApiRequirement & {
    type?: 'functional' | 'technical' | 'non_functional';
    description?: string;
};

interface RequirementsTabProps {
    requirements: RequirementDisplay[];
    rawTasks?: Task[];
    onAddRequirement?: () => void;
    onRequirementClick?: (req: RequirementDisplay) => void;
    onStatusChange?: (reqId: string, newStatus: RequirementStatus) => void | Promise<void>;
    onAnalyzeWithAI?: () => void;
    onImport?: () => void;
}

const statusStyles: Record<string, { bg: string; text: string; icon: string }> = {
    todo: { bg: 'bg-muted', text: 'text-muted-foreground', icon: 'radio_button_unchecked' },
    in_progress: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', icon: 'progress_activity' },
    done: { bg: 'bg-green-100 dark:bg-green-500/20', text: 'text-green-600 dark:text-green-400', icon: 'check_circle' },
};

const STATUS_OPTIONS: RequirementStatus[] = ['todo', 'in_progress', 'done'];
const STATUS_LABELS: Record<RequirementStatus, string> = {
    todo: 'Todo',
    in_progress: 'In Progress',
    done: 'Done',
};

function getDueDateBadge(dueDate: string | null | undefined): { label: string; className: string } | null {
    if (!dueDate) return null;
    const due = new Date(dueDate + 'T12:00:00');
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    due.setHours(0, 0, 0, 0);
    const diffDays = Math.ceil((due.getTime() - today.getTime()) / (1000 * 60 * 60 * 24));
    if (diffDays < 0) return { label: 'Overdue', className: 'bg-red-500/20 text-red-600 dark:text-red-400' };
    if (diffDays <= 7) return { label: 'Due soon', className: 'bg-amber-500/20 text-amber-600 dark:text-amber-400' };
    return null;
}

const DESCRIPTION_PREVIEW_LENGTH = 80;

export function RequirementsTab({ requirements, rawTasks = [], onAddRequirement, onRequirementClick, onStatusChange, onAnalyzeWithAI, onImport }: RequirementsTabProps) {
    const [statusDropdownReqId, setStatusDropdownReqId] = useState<string | null>(null);
    const [expandedReqIds, setExpandedReqIds] = useState<Set<string>>(new Set());

    const getLinkedTasksCount = (reqId: string) => {
        return rawTasks.filter(t => t.requirement_id === reqId).length;
    };
    const getLinkedTasksDoneCount = (reqId: string) => {
        return rawTasks.filter(t => t.requirement_id === reqId && ['done', 'archived', 'cancelled', 'canceled'].includes((t.status || '').toLowerCase())).length;
    };
    return (
        <div className="flex flex-col gap-4">
            <div className="bg-card border border-border rounded-lg overflow-hidden">
                <div className="px-4 py-3 border-b border-border flex justify-between items-center bg-muted/50">
                    <h3 className="text-sm font-bold text-card-foreground">Project Requirements Document (PRD)</h3>
                    <div className="flex gap-1.5">
                        <button
                            onClick={onImport}
                            className="flex items-center gap-1.5 px-2.5 py-1 bg-card border border-border text-card-foreground text-xs font-medium rounded hover:bg-muted transition-colors"
                        >
                            <span className="material-symbols-outlined text-[16px]">upload</span>
                            Import
                        </button>
                        <button
                            onClick={onAnalyzeWithAI}
                            className="flex items-center gap-1.5 px-2.5 py-1 bg-primary text-primary-foreground text-xs font-bold rounded hover:bg-primary/90 transition-colors shadow-md shadow-primary/20"
                        >
                            <span className="material-symbols-outlined text-[16px]">auto_fix</span>
                            Analyze with AI
                        </button>
                    </div>
                </div>

                <div className="divide-y divide-border">
                    {requirements.length === 0 ? (
                        <div className="p-6 text-center">
                            <span className="material-symbols-outlined text-muted-foreground/50 text-3xl mb-1.5">description</span>
                            <p className="text-muted-foreground text-xs">No requirements yet</p>
                            <p className="text-muted-foreground/70 text-[10px] mt-0.5">Add requirements to define project scope</p>
                        </div>
                    ) : requirements.map((req) => {
                        const statusStyle = statusStyles[req.status] || statusStyles.todo;
                        const priorityColor = req.priority === 'critical' ? 'text-red-500' :
                            req.priority === 'high' ? 'text-orange-500' : 'text-blue-500';
                        // Use content or description (for backward compatibility)
                        const displayContent = req.content || req.description || '';
                        const isLong = displayContent.length > DESCRIPTION_PREVIEW_LENGTH;
                        const isExpanded = expandedReqIds.has(req.id);
                        const preview = isExpanded ? displayContent : (displayContent.slice(0, DESCRIPTION_PREVIEW_LENGTH) + (isLong ? '…' : ''));
                        // Truncate ID for display
                        const shortId = req.id.slice(0, 8);
                        const linkedCount = getLinkedTasksCount(req.id);
                        const doneCount = getLinkedTasksDoneCount(req.id);
                        const isStatusDropdownOpen = statusDropdownReqId === req.id;

                        const toggleExpand = (e: React.MouseEvent) => {
                            e.stopPropagation();
                            setExpandedReqIds((prev) => {
                                const next = new Set(prev);
                                if (next.has(req.id)) next.delete(req.id);
                                else next.add(req.id);
                                return next;
                            });
                        };

                        return (
                            <div
                                key={req.id}
                                onClick={() => onRequirementClick?.(req)}
                                className="p-3 hover:bg-muted/50 transition-colors flex flex-col sm:flex-row sm:items-start gap-3 group cursor-pointer"
                            >
                                <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-2 mb-0.5">
                                        <span className="text-[10px] font-mono text-muted-foreground">#{shortId}</span>
                                        <h4 className="text-xs font-bold text-card-foreground truncate">{req.title}</h4>
                                    </div>
                                    <p className={`text-xs text-muted-foreground leading-snug ${isExpanded ? 'whitespace-pre-wrap' : ''}`}>
                                        {preview}
                                        {isLong && (
                                            <button
                                                type="button"
                                                onClick={toggleExpand}
                                                className="ml-1 text-sky-400 hover:text-sky-300 hover:underline text-[10px] font-medium"
                                            >
                                                {isExpanded ? 'Collapse' : 'Show more'}
                                            </button>
                                        )}
                                    </p>
                                    {linkedCount > 0 && (
                                        <div className="mt-1.5 flex items-center gap-1.5 text-[10px] text-muted-foreground">
                                            <span className="material-symbols-outlined text-[12px]">task_alt</span>
                                            {doneCount}/{linkedCount} tasks
                                        </div>
                                    )}
                                    {linkedCount === 0 && (
                                        <div className="mt-1.5 text-[10px] text-muted-foreground/60">No tasks yet</div>
                                    )}
                                    {req.due_date && (
                                        <div className="mt-1.5 flex items-center gap-1.5">
                                            <span className="text-[10px] text-muted-foreground">
                                                Due: {new Date(req.due_date + 'T12:00:00').toLocaleDateString()}
                                            </span>
                                            {(() => {
                                                const badge = getDueDateBadge(req.due_date);
                                                return badge ? (
                                                    <span className={`text-[9px] font-bold px-1 py-0.5 rounded ${badge.className}`}>
                                                        {badge.label}
                                                    </span>
                                                ) : null;
                                            })()}
                                        </div>
                                    )}
                                </div>
                                <div className="flex items-center gap-3 shrink-0">
                                    <div className="flex flex-col items-end gap-0.5">
                                        <span className={`text-[10px] font-bold capitalize ${priorityColor}`}>{req.priority}</span>
                                        <span className="text-[9px] text-muted-foreground uppercase tracking-wide">Priority</span>
                                    </div>
                                    <div className="relative" onClick={(e) => e.stopPropagation()}>
                                        <button
                                            onClick={() => setStatusDropdownReqId(isStatusDropdownOpen ? null : req.id)}
                                            className={`flex items-center gap-1 text-[10px] font-bold px-1.5 py-0.5 rounded-full ${statusStyle.bg} ${statusStyle.text} hover:opacity-90`}
                                        >
                                            <span className="material-symbols-outlined text-[12px]">{statusStyle.icon}</span>
                                            {STATUS_LABELS[req.status]}
                                            <span className="material-symbols-outlined text-[12px]">expand_more</span>
                                        </button>
                                        {isStatusDropdownOpen && (
                                            <>
                                                <div className="fixed inset-0 z-10" onClick={() => setStatusDropdownReqId(null)} />
                                                <div className="absolute right-0 top-full mt-1 z-20 py-1 bg-card border border-border rounded shadow-lg min-w-[100px]">
                                                    {STATUS_OPTIONS.map((s) => (
                                                        <button
                                                            key={s}
                                                            onClick={async () => {
                                                                await onStatusChange?.(req.id, s);
                                                                setStatusDropdownReqId(null);
                                                            }}
                                                            className="w-full px-2 py-1.5 text-left text-xs hover:bg-muted flex items-center gap-1.5"
                                                        >
                                                            <span className="material-symbols-outlined text-[14px]">
                                                                {statusStyles[s]?.icon || 'circle'}
                                                            </span>
                                                            {STATUS_LABELS[s]}
                                                        </button>
                                                    ))}
                                                </div>
                                            </>
                                        )}
                                    </div>
                                </div>
                            </div>
                        );
                    })}
                </div>

                <div className="p-2 bg-muted/50 border-t border-border text-center">
                    <button
                        onClick={onAddRequirement}
                        className="text-xs text-muted-foreground hover:text-primary font-medium transition-colors flex items-center justify-center gap-1.5"
                    >
                        <span className="material-symbols-outlined text-[16px]">add</span>
                        Add New Requirement
                    </button>
                </div>
            </div>
        </div>
    );
}
