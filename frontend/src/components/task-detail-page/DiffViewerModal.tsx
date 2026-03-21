import { useState } from 'react';
import type { Task } from '../../api/tasks';
import type { TaskContext } from '../../api/taskContexts';
import { approveAttempt } from '../../api/taskAttempts';
import { DiffViewer } from '../diff-viewer';
import { TaskDocumentPreview } from './TaskDocumentPreview';
import { isTaskDocumentPreview } from '../../lib/taskDocuments';

interface DiffViewerModalProps {
    attemptId: string;
    taskStatus: string;
    task?: Task | null;
    taskContexts?: TaskContext[];
    previewMetadata?: Record<string, unknown>;
    focusFilePath?: string;
    singleFileMode?: boolean;
    onClose: () => void;
    onApproved: () => void;
}

export function DiffViewerModal({
    attemptId,
    taskStatus,
    task,
    taskContexts = [],
    previewMetadata,
    focusFilePath,
    singleFileMode = false,
    onClose,
    onApproved
}: DiffViewerModalProps) {
    const [isApproving, setIsApproving] = useState(false);
    const [approveError, setApproveError] = useState<string | null>(null);
    const showDocumentReview = Boolean(
        task && isTaskDocumentPreview(task.task_type, previewMetadata ?? task.metadata),
    );

    const handleApprove = async () => {
        if (!task || isApproving) {
            return;
        }

        setApproveError(null);
        setIsApproving(true);
        try {
            await approveAttempt(attemptId);
            onApproved();
        } catch (error) {
            setApproveError(
                error instanceof Error ? error.message : 'Failed to approve document review.',
            );
        } finally {
            setIsApproving(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
            <div className="absolute inset-0 bg-black/80" onClick={onClose}></div>
            <div className="relative w-full max-w-5xl bg-background border border-border rounded-sm shadow-2xl overflow-hidden max-h-[90vh] flex flex-col">
                <div className="px-4 py-3 border-b border-border flex justify-between items-center bg-muted/30">
                    <div className="flex items-center gap-3">
                        <span className="material-symbols-outlined text-amber-500">rate_review</span>
                        <h3 className="text-base font-semibold text-foreground">
                            {showDocumentReview ? 'Review Document' : 'Review Changes'}
                        </h3>
                    </div>
                    <button
                        onClick={onClose}
                        className="h-8 w-8 inline-flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors rounded-sm hover:bg-muted/50"
                    >
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>
                <div className="flex-1 overflow-y-auto p-3">
                    {showDocumentReview && task ? (
                        <div className="space-y-4">
                            <TaskDocumentPreview
                                task={task}
                                taskContexts={taskContexts}
                                metadata={previewMetadata}
                                isReviewMode
                            />
                            {approveError && (
                                <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
                                    {approveError}
                                </div>
                            )}
                        </div>
                    ) : (
                        <DiffViewer
                            attemptId={attemptId}
                            taskTitle={`Attempt (${taskStatus})`}
                            focusFilePath={focusFilePath}
                            showOnlyFocusedFile={singleFileMode}
                            hideReviewActions
                            onActionComplete={onApproved}
                        />
                    )}
                </div>
                {showDocumentReview && taskStatus === 'in_review' && (
                    <div className="border-t border-border bg-muted/30 px-4 py-3 flex justify-end gap-3">
                        <button
                            type="button"
                            onClick={onClose}
                            className="rounded-lg border border-border px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground"
                        >
                            Close
                        </button>
                        <button
                            type="button"
                            onClick={() => void handleApprove()}
                            disabled={isApproving}
                            className="inline-flex items-center gap-2 rounded-lg bg-emerald-500 px-4 py-2 text-sm font-semibold text-white hover:bg-emerald-600 disabled:cursor-not-allowed disabled:opacity-60"
                        >
                            {isApproving ? (
                                <span className="h-4 w-4 rounded-full border-2 border-white/30 border-t-white animate-spin" />
                            ) : (
                                <span className="material-symbols-outlined text-[16px]">check</span>
                            )}
                            {isApproving ? 'Approving...' : 'Approve Document'}
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
