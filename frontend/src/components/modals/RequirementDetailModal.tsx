// RequirementDetailModal — View requirement detail, linked tasks, actions
import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import {
    updateRequirement,
    deleteRequirement,
    getRequirementAttachmentDownloadUrl,
    startRequirementTaskSequence,
    type Requirement,
    type RequirementStatus,
} from '../../api/requirements';
import type { Task } from '../../shared/types';
import { ConfirmModal } from './ConfirmModal';
import { logger } from '@/lib/logger';

interface AttachmentMeta {
    key: string;
    filename: string;
    content_type: string;
    size: number;
    uploaded_at?: string;
}

function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function isImageType(ct: string): boolean {
    return ct.startsWith('image/');
}

function AttachmentDisplay({
    projectId,
    attachment,
}: {
    projectId: string;
    attachment: AttachmentMeta;
}) {
    const [downloadUrl, setDownloadUrl] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [imageExpanded, setImageExpanded] = useState(false);

    useEffect(() => {
        let cancelled = false;
        getRequirementAttachmentDownloadUrl(projectId, { key: attachment.key })
            .then((res) => {
                if (!cancelled) setDownloadUrl(res.download_url);
            })
            .catch((err) => {
                if (!cancelled) setError(err instanceof Error ? err.message : 'Failed to load');
            });
        return () => {
            cancelled = true;
        };
    }, [projectId, attachment.key]);

    const isImage = isImageType(attachment.content_type);

    if (error) {
        return (
            <div className="p-3 rounded-lg bg-muted/50 border border-border">
                <span className="material-symbols-outlined text-destructive text-[20px]">error</span>
                <p className="text-xs text-muted-foreground truncate mt-1">{attachment.filename}</p>
            </div>
        );
    }

    if (isImage && downloadUrl) {
        return (
            <div>
                <div
                    onClick={() => setImageExpanded(true)}
                    className="aspect-square rounded-lg overflow-hidden border border-border bg-muted/50 cursor-pointer hover:ring-2 hover:ring-primary"
                >
                    <img
                        src={downloadUrl}
                        alt={attachment.filename}
                        className="w-full h-full object-cover"
                    />
                </div>
                {imageExpanded && (
                    <div
                        className="fixed inset-0 z-[60] flex items-center justify-center bg-black/80 p-4"
                        onClick={() => setImageExpanded(false)}
                    >
                        <img
                            src={downloadUrl}
                            alt={attachment.filename}
                            className="max-w-full max-h-full object-contain"
                            onClick={(e) => e.stopPropagation()}
                        />
                    </div>
                )}
            </div>
        );
    }

    if (downloadUrl) {
        return (
            <a
                href={downloadUrl}
                target="_blank"
                rel="noopener noreferrer"
                download={attachment.filename}
                className="flex items-center gap-2 p-3 rounded-lg bg-muted/50 border border-border hover:bg-muted transition-colors"
            >
                <span className="material-symbols-outlined text-muted-foreground text-[20px]">description</span>
                <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium truncate">{attachment.filename}</p>
                    <p className="text-xs text-muted-foreground">{formatBytes(attachment.size)}</p>
                </div>
                <span className="material-symbols-outlined text-muted-foreground text-[18px]">download</span>
            </a>
        );
    }

    return (
        <div className="flex items-center gap-2 p-3 rounded-lg bg-muted/50 border border-border">
            <span className="material-symbols-outlined text-muted-foreground text-[20px] animate-pulse">hourglass_empty</span>
            <div className="min-w-0 flex-1">
                <p className="text-sm truncate">{attachment.filename}</p>
                <p className="text-xs text-muted-foreground">{formatBytes(attachment.size)}</p>
            </div>
        </div>
    );
}

const STATUS_OPTIONS: RequirementStatus[] = ['todo', 'in_progress', 'done'];
const STATUS_LABELS: Record<RequirementStatus, string> = {
    todo: 'Todo',
    in_progress: 'In Progress',
    done: 'Done',
};

const statusStyles: Record<string, { bg: string; text: string; icon: string }> = {
    todo: { bg: 'bg-muted', text: 'text-muted-foreground', icon: 'radio_button_unchecked' },
    in_progress: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', icon: 'progress_activity' },
    done: { bg: 'bg-green-100 dark:bg-green-500/20', text: 'text-green-600 dark:text-green-400', icon: 'check_circle' },
};

const taskStatusIcons: Record<string, string> = {
    todo: 'radio_button_unchecked',
    in_progress: 'progress_activity',
    in_review: 'rate_review',
    done: 'check_circle',
};

function normalizeTaskStatus(status: string | undefined): string {
    if (!status) return 'todo';
    return status
        .trim()
        .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
        .replace(/[\s-]+/g, '_')
        .toLowerCase();
}

function isAiBreakdownAnalysisTask(task: Task): boolean {
    const metadata = task.metadata || {};
    const mode = String(metadata.breakdown_mode ?? '').trim().toLowerCase();
    const kind = String(metadata.breakdown_kind ?? '').trim().toLowerCase();
    return (
        mode === 'ai_support' ||
        kind === 'analysis_session' ||
        task.title?.startsWith('[Breakdown]') === true
    );
}

function dedupeLinkedTasks(tasks: Task[]): Task[] {
    const sorted = [...tasks].sort((a, b) => {
        const aTs = Date.parse(a.updated_at || a.created_at || '') || 0;
        const bTs = Date.parse(b.updated_at || b.created_at || '') || 0;
        return bTs - aTs;
    });
    const kept: Task[] = [];
    const seenBreakdownTitles = new Set<string>();
    for (const task of sorted) {
        if (!isAiBreakdownAnalysisTask(task)) {
            kept.push(task);
            continue;
        }
        const key = task.title.trim().toLowerCase();
        if (seenBreakdownTitles.has(key)) {
            continue;
        }
        seenBreakdownTitles.add(key);
        kept.push(task);
    }
    return kept;
}

function taskStatusEligibleForSequence(task: Task): boolean {
    const normalized = normalizeTaskStatus(task.status);
    return normalized === 'todo' || normalized === 'in_progress';
}

interface RequirementDetailModalProps {
    isOpen: boolean;
    onClose: () => void;
    projectId: string;
    requirement: Requirement | null;
    linkedTasks: Task[];
    onEdit: () => void;
    onRefresh: () => void;
    onBreakIntoTasks?: () => void;
}

export function RequirementDetailModal({
    isOpen,
    onClose,
    projectId,
    requirement,
    linkedTasks,
    onEdit,
    onRefresh,
    onBreakIntoTasks,
}: RequirementDetailModalProps) {
    const navigate = useNavigate();
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const [isDeleting, setIsDeleting] = useState(false);
    const [statusDropdownOpen, setStatusDropdownOpen] = useState(false);
    const [isUpdatingStatus, setIsUpdatingStatus] = useState(false);
    const [isStartingSequence, setIsStartingSequence] = useState(false);
    const [sequenceMessage, setSequenceMessage] = useState<string | null>(null);
    const [sequenceError, setSequenceError] = useState<string | null>(null);

    useEffect(() => {
        if (!isOpen) return;
        setSequenceMessage(null);
        setSequenceError(null);
        setIsStartingSequence(false);
    }, [isOpen, requirement?.id]);

    if (!isOpen || !requirement) return null;

    const visibleLinkedTasks = dedupeLinkedTasks(linkedTasks);
    const sequenceEligibleTasks = visibleLinkedTasks.filter(taskStatusEligibleForSequence);
    const linkedTaskStatuses = visibleLinkedTasks.map((task) => normalizeTaskStatus(task.status));
    const linkedDoneCount = linkedTaskStatuses.filter((status) =>
        ['done', 'archived', 'cancelled', 'canceled'].includes(status)
    ).length;
    const hasLinkedTasks = visibleLinkedTasks.length > 0;
    const derivedRequirementStatus: RequirementStatus =
        !hasLinkedTasks
            ? requirement.status
            : linkedDoneCount === linkedTaskStatuses.length
                ? 'done'
                : linkedDoneCount > 0 ||
                    linkedTaskStatuses.some((status) => ['in_progress', 'in_review', 'blocked'].includes(status))
                    ? 'in_progress'
                    : 'todo';
    const statusManagedByTasks = hasLinkedTasks;

    const shortId = requirement.id.slice(0, 8);
    const statusStyle = statusStyles[derivedRequirementStatus] || statusStyles.todo;
    const isInProgressStatus = derivedRequirementStatus === 'in_progress';
    const createdDate = requirement.created_at ? new Date(requirement.created_at).toLocaleDateString() : '';

    const handleDelete = async () => {
        setIsDeleting(true);
        try {
            await deleteRequirement(projectId, requirement.id);
            onRefresh();
            onClose();
        } catch (err) {
            logger.error('Failed to delete requirement:', err);
        } finally {
            setIsDeleting(false);
            setShowDeleteConfirm(false);
        }
    };

    const handleStatusChange = async (newStatus: RequirementStatus) => {
        if (statusManagedByTasks) return;
        setStatusDropdownOpen(false);
        if (newStatus === requirement.status) return;
        setIsUpdatingStatus(true);
        try {
            await updateRequirement(projectId, requirement.id, { status: newStatus });
            onRefresh();
        } catch (err) {
            logger.error('Failed to update status:', err);
        } finally {
            setIsUpdatingStatus(false);
        }
    };

    const handleTaskClick = (taskId: string) => {
        navigate(`/projects/${projectId}/task/${taskId}`);
    };

    const handleStartRequirementSequence = async () => {
        if (sequenceEligibleTasks.length === 0) {
            setSequenceError('No todo/in-progress linked tasks available for sequential start.');
            return;
        }

        setIsStartingSequence(true);
        setSequenceError(null);
        setSequenceMessage(null);
        try {
            const response = await startRequirementTaskSequence(projectId, requirement.id);
            if (response.total_tasks === 0) {
                setSequenceMessage('No eligible tasks to start.');
            } else {
                setSequenceMessage(
                    `Sequential run started (${response.total_tasks} task(s)). Tasks will execute one-by-one.`
                );
            }
            onRefresh();
        } catch (err) {
            setSequenceError(
                err instanceof Error ? err.message : 'Failed to start linked tasks sequentially'
            );
        } finally {
            setIsStartingSequence(false);
        }
    };

    return (
        <>
            <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
                <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose}></div>
                <div className="relative w-full max-w-2xl max-h-[90vh] overflow-hidden bg-card border border-border rounded-2xl shadow-2xl flex flex-col">
                    {/* Header */}
                    <div className="px-6 py-5 border-b border-border flex justify-between items-start bg-muted shrink-0">
                        <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2 mb-1">
                                <span className="text-xs font-mono text-muted-foreground">#{shortId}</span>
                                <h2 className="text-lg font-bold text-card-foreground truncate">{requirement.title}</h2>
                            </div>
                            <div className="flex items-center gap-3 flex-wrap">
                                <span className="text-sm text-muted-foreground capitalize">Priority: {requirement.priority}</span>
                                {createdDate && (
                                    <span className="text-sm text-muted-foreground">Created: {createdDate}</span>
                                )}
                                {requirement.due_date && (
                                    <span className="text-sm text-muted-foreground">
                                        Due: {new Date(requirement.due_date + 'T12:00:00').toLocaleDateString()}
                                    </span>
                                )}
                                {statusManagedByTasks && (
                                    <span className="text-xs text-muted-foreground bg-card/60 border border-border rounded-full px-2 py-0.5">
                                        Status follows linked tasks
                                    </span>
                                )}
                            </div>
                        </div>
                        <div className="flex items-center gap-2 shrink-0">
                            <div className="relative">
                                <button
                                    onClick={() => {
                                        if (statusManagedByTasks) return;
                                        setStatusDropdownOpen(!statusDropdownOpen);
                                    }}
                                    disabled={isUpdatingStatus || statusManagedByTasks}
                                    className={`flex items-center gap-1 text-xs font-bold px-3 py-1.5 rounded-full ${statusStyle.bg} ${statusStyle.text} hover:opacity-90 transition-opacity`}
                                    title={statusManagedByTasks ? 'Status is auto-derived from linked tasks' : 'Change requirement status'}
                                >
                                    {isInProgressStatus ? (
                                        <span className="inline-block w-3.5 h-3.5 rounded-full border-2 border-current/35 border-t-current animate-spin" />
                                    ) : (
                                        <span className="material-symbols-outlined text-[14px]">{statusStyle.icon}</span>
                                    )}
                                    {STATUS_LABELS[derivedRequirementStatus]}
                                    {!statusManagedByTasks && (
                                        <span className="material-symbols-outlined text-[14px]">expand_more</span>
                                    )}
                                </button>
                                {statusDropdownOpen && !statusManagedByTasks && (
                                    <>
                                        <div className="fixed inset-0 z-10" onClick={() => setStatusDropdownOpen(false)} />
                                        <div className="absolute right-0 top-full mt-1 z-20 py-1 bg-card border border-border rounded-lg shadow-lg min-w-[140px]">
                                            {STATUS_OPTIONS.map((s) => (
                                                <button
                                                    key={s}
                                                    onClick={() => handleStatusChange(s)}
                                                    className="w-full px-3 py-2 text-left text-sm hover:bg-muted flex items-center gap-2"
                                                >
                                                    <span className="material-symbols-outlined text-[16px]">
                                                        {statusStyles[s]?.icon || 'circle'}
                                                    </span>
                                                    {STATUS_LABELS[s]}
                                                </button>
                                            ))}
                                        </div>
                                    </>
                                )}
                            </div>
                            <button onClick={onClose} className="text-muted-foreground hover:text-card-foreground transition-colors">
                                <span className="material-symbols-outlined">close</span>
                            </button>
                        </div>
                    </div>

                    {/* Body - scrollable */}
                    <div className="flex-1 overflow-y-auto p-6">
                        {/* Content */}
                        <div className="mb-6">
                            {(() => {
                                const marker = '\n\n---\nAcceptance criteria:\n';
                                const idx = (requirement.content || '').indexOf(marker);
                                const hasCriteria = idx >= 0;
                                const desc = hasCriteria ? requirement.content!.slice(0, idx).trim() : (requirement.content || '').trim();
                                const criteria = hasCriteria ? requirement.content!.slice(idx + marker.length).trim() : '';
                                return (
                                    <>
                                        <h3 className="text-sm font-bold text-card-foreground mb-2">Description</h3>
                                        <div className="text-sm text-muted-foreground whitespace-pre-wrap bg-muted/50 rounded-lg p-4 mb-4">
                                            {desc || 'No description'}
                                        </div>
                                        {criteria && (
                                            <>
                                                <h3 className="text-sm font-bold text-card-foreground mb-2">Acceptance Criteria</h3>
                                                <div className="text-sm text-muted-foreground whitespace-pre-wrap bg-muted/50 rounded-lg p-4 font-mono text-[13px]">
                                                    {criteria}
                                                </div>
                                            </>
                                        )}
                                    </>
                                );
                            })()}
                        </div>

                        {/* Attachments */}
                        {(() => {
                            const attachments = (requirement.metadata?.attachments as AttachmentMeta[] | undefined) || [];
                            if (attachments.length === 0) return null;
                            return (
                                <div className="mb-6">
                                    <h3 className="text-sm font-bold text-card-foreground mb-2">
                                        Reference Files ({attachments.length})
                                    </h3>
                                    <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
                                        {attachments.map((a) => (
                                            <AttachmentDisplay
                                                key={a.key}
                                                projectId={projectId}
                                                attachment={a}
                                            />
                                        ))}
                                    </div>
                                </div>
                            );
                        })()}

                        {/* Linked Tasks */}
                        <div>
                            <h3 className="text-sm font-bold text-card-foreground mb-2">
                                Linked Tasks ({visibleLinkedTasks.length})
                            </h3>
                            {visibleLinkedTasks.length > 0 && (
                                <p className="text-xs text-muted-foreground mb-2">
                                    Eligible for sequential start: {sequenceEligibleTasks.length}
                                </p>
                            )}
                            {sequenceMessage && (
                                <div className="mb-2 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-400">
                                    {sequenceMessage}
                                </div>
                            )}
                            {sequenceError && (
                                <div className="mb-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-400">
                                    {sequenceError}
                                </div>
                            )}
                            {visibleLinkedTasks.length === 0 ? (
                                <div className="text-sm text-muted-foreground bg-muted/30 rounded-lg p-4">
                                    No tasks linked yet.
                                    {onBreakIntoTasks && (
                                        <span> Click &quot;Break into Tasks&quot; to create tasks from this requirement.</span>
                                    )}
                                </div>
                            ) : (
                                <div className="space-y-2">
                                    {visibleLinkedTasks.map((task) => {
                                        const taskStatus = normalizeTaskStatus(task.status);
                                        const icon = taskStatusIcons[taskStatus] || 'radio_button_unchecked';
                                        const isDone = ['done', 'archived', 'cancelled', 'canceled'].includes(taskStatus);
                                        return (
                                            <div
                                                key={task.id}
                                                onClick={() => handleTaskClick(task.id)}
                                                className="flex items-center gap-3 p-3 bg-muted/50 rounded-lg hover:bg-muted transition-colors cursor-pointer"
                                            >
                                                <span className={`material-symbols-outlined ${isDone ? 'text-green-500' : 'text-muted-foreground'}`}>
                                                    {isDone ? 'check_circle' : icon}
                                                </span>
                                                <div className="flex-1 min-w-0">
                                                    <p className="text-sm font-medium text-card-foreground truncate">{task.title}</p>
                                                    <p className="text-xs text-muted-foreground capitalize">
                                                        {taskStatus.replace(/_/g, ' ')} · {task.task_type || 'feature'}
                                                    </p>
                                                </div>
                                                <span className="material-symbols-outlined text-muted-foreground text-[18px]">
                                                    arrow_forward
                                                </span>
                                            </div>
                                        );
                                    })}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Actions */}
                    <div className="px-6 py-4 border-t border-border bg-muted flex flex-wrap gap-2 shrink-0">
                        {onBreakIntoTasks && (
                            <button
                                onClick={onBreakIntoTasks}
                                className="flex items-center gap-2 px-4 py-2 bg-primary text-primary-foreground text-sm font-bold rounded-lg hover:bg-primary/90"
                            >
                                <span className="material-symbols-outlined text-[18px]">auto_fix</span>
                                Break into Tasks
                            </button>
                        )}
                        <button
                            onClick={handleStartRequirementSequence}
                            disabled={isStartingSequence || sequenceEligibleTasks.length === 0}
                            className="flex items-center gap-2 px-4 py-2 bg-blue-500/15 text-blue-400 text-sm font-semibold rounded-lg hover:bg-blue-500/25 disabled:opacity-50 disabled:cursor-not-allowed"
                            title={
                                sequenceEligibleTasks.length === 0
                                    ? 'No todo/in-progress linked tasks to start'
                                    : 'Start linked tasks one-by-one'
                            }
                        >
                            <span
                                className={`material-symbols-outlined text-[18px] ${
                                    isStartingSequence ? 'animate-spin' : ''
                                }`}
                            >
                                {isStartingSequence ? 'progress_activity' : 'playlist_play'}
                            </span>
                            {isStartingSequence ? 'Starting...' : 'Start All (Sequential)'}
                        </button>
                        <button
                            onClick={onEdit}
                            className="flex items-center gap-2 px-4 py-2 bg-card border border-border text-card-foreground text-sm font-medium rounded-lg hover:bg-muted"
                        >
                            <span className="material-symbols-outlined text-[18px]">edit</span>
                            Edit
                        </button>
                        <button
                            onClick={() => setShowDeleteConfirm(true)}
                            className="flex items-center gap-2 px-4 py-2 bg-red-500/10 text-red-600 dark:text-red-400 text-sm font-medium rounded-lg hover:bg-red-500/20"
                        >
                            <span className="material-symbols-outlined text-[18px]">delete</span>
                            Delete
                        </button>
                    </div>
                </div>
            </div>

            <ConfirmModal
                isOpen={showDeleteConfirm}
                onClose={() => setShowDeleteConfirm(false)}
                onConfirm={handleDelete}
                title="Delete Requirement"
                message={
                    linkedTasks.length > 0
                        ? `This requirement has ${linkedTasks.length} linked task(s). Deleting will unlink them (tasks won't be deleted). Are you sure?`
                        : 'Are you sure you want to delete this requirement?'
                }
                confirmText="Delete"
                confirmVariant="danger"
                isLoading={isDeleting}
            />
        </>
    );
}
