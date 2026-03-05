import { useEffect, useState } from 'react';

interface ConfigureAgentModalProps {
    isOpen: boolean;
    onClose: () => void;
    taskId: string;
    taskTitle: string;
    onStart?: () => Promise<void> | void;
}

export function ConfigureAgentModal({
    isOpen,
    onClose,
    taskId,
    taskTitle,
    onStart,
}: ConfigureAgentModalProps) {
    const [isStarting, setIsStarting] = useState(false);
    const [submitError, setSubmitError] = useState<string | null>(null);

    useEffect(() => {
        if (!isOpen) return;
        setIsStarting(false);
        setSubmitError(null);
    }, [isOpen, taskId]);

    if (!isOpen) return null;

    const handleStart = async () => {
        if (isStarting || !onStart) return;

        try {
            setIsStarting(true);
            setSubmitError(null);
            await onStart();
            onClose();
        } catch (error) {
            setSubmitError(error instanceof Error ? error.message : 'Failed to start task execution.');
        } finally {
            setIsStarting(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
            <div
                className="absolute inset-0 bg-black/70 backdrop-blur-[2px] transition-opacity"
                onClick={() => {
                    if (!isStarting) onClose();
                }}
            />
            <div className="relative w-full max-w-lg bg-card border border-border rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
                <div className="px-6 py-5 border-b border-border/80 flex justify-between items-start bg-muted/30">
                    <div>
                        <h2 className="text-lg font-bold text-foreground flex items-center gap-2">
                            <span className="material-symbols-outlined text-primary">smart_toy</span>
                            Start Agent Execution
                        </h2>
                        <p className="text-sm text-muted-foreground mt-1 font-mono">
                            Create a new attempt for task{' '}
                            <span className="text-foreground font-semibold">{taskId.slice(0, 8)}</span>
                        </p>
                    </div>
                    <button
                        onClick={onClose}
                        disabled={isStarting}
                        className="text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                <div className="p-6 overflow-y-auto bg-card space-y-4">
                    <div className="p-4 rounded-xl bg-muted/40 border border-border/70">
                        <p className="text-xs text-muted-foreground mb-1">Task</p>
                        <p className="text-sm font-medium text-foreground leading-relaxed break-words">
                            {taskTitle}
                        </p>
                    </div>

                    <div className="p-4 rounded-xl bg-muted/30 border border-border/70">
                        <div className="flex items-center gap-2 mb-2">
                            <span className="material-symbols-outlined text-sm text-primary">info</span>
                            <p className="text-xs font-semibold text-foreground uppercase tracking-wide">
                                Execution Notes
                            </p>
                        </div>
                        <ul className="text-sm text-muted-foreground space-y-1">
                            <li>Agent profile and strategy are controlled by global Settings.</li>
                            <li>Starting will create a new attempt and move task to In Progress.</li>
                            <li>Use View Logs to monitor execution details after start.</li>
                        </ul>
                    </div>

                    {submitError ? (
                        <div className="p-3 rounded-lg bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-300 text-sm">
                            {submitError}
                        </div>
                    ) : null}
                </div>

                <div className="px-6 py-4 border-t border-border/80 bg-muted/20 flex justify-end gap-3">
                    <button
                        onClick={onClose}
                        disabled={isStarting}
                        className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={handleStart}
                        disabled={isStarting || !onStart}
                        className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 hover:shadow-primary/40 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        {isStarting ? (
                            <>
                                <span className="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent" />
                                Starting...
                            </>
                        ) : (
                            <>
                                <span className="material-symbols-outlined text-[18px] material-symbols-filled">
                                    play_arrow
                                </span>
                                Start Agent
                            </>
                        )}
                    </button>
                </div>
            </div>
        </div>
    );
}
