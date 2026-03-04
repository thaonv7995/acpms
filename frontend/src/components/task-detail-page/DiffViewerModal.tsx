import { DiffViewer } from '../diff-viewer';

interface DiffViewerModalProps {
    attemptId: string;
    taskStatus: string;
    focusFilePath?: string;
    singleFileMode?: boolean;
    onClose: () => void;
    onApproved: () => void;
}

export function DiffViewerModal({
    attemptId,
    taskStatus,
    focusFilePath,
    singleFileMode = false,
    onClose,
    onApproved
}: DiffViewerModalProps) {
    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
            <div className="absolute inset-0 bg-black/80" onClick={onClose}></div>
            <div className="relative w-full max-w-5xl bg-background border border-border rounded-sm shadow-2xl overflow-hidden max-h-[90vh] flex flex-col">
                <div className="px-4 py-3 border-b border-border flex justify-between items-center bg-muted/30">
                    <div className="flex items-center gap-3">
                        <span className="material-symbols-outlined text-amber-500">rate_review</span>
                        <h3 className="text-base font-semibold text-foreground">Review Changes</h3>
                    </div>
                    <button
                        onClick={onClose}
                        className="h-8 w-8 inline-flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors rounded-sm hover:bg-muted/50"
                    >
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>
                <div className="flex-1 overflow-y-auto p-3">
                    <DiffViewer
                        attemptId={attemptId}
                        taskTitle={`Attempt (${taskStatus})`}
                        focusFilePath={focusFilePath}
                        showOnlyFocusedFile={singleFileMode}
                        hideReviewActions
                        onActionComplete={onApproved}
                    />
                </div>
            </div>
        </div>
    );
}
