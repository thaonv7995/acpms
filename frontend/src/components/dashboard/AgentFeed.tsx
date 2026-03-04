import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { TimelineLogDisplay } from '../timeline-log';
import { useAgentLogs } from '../../hooks/useAgentLogs';

export function AgentFeed() {
    const { statuses, loading, error } = useAgentLogs();
    const [selectedAttempt, setSelectedAttempt] = useState<string | null>(null);

    const runningAttempts = useMemo(
        () =>
            statuses
                .filter((status) => status.status === 'running')
                .sort(
                    (a, b) =>
                        new Date(b.started_at || b.created_at).getTime() -
                        new Date(a.started_at || a.created_at).getTime()
                ),
        [statuses]
    );

    useEffect(() => {
        if (runningAttempts.length === 0) {
            setSelectedAttempt(null);
            return;
        }

        const existing = selectedAttempt
            ? runningAttempts.find((attempt) => attempt.id === selectedAttempt)
            : null;
        if (!existing) {
            setSelectedAttempt(runningAttempts[0].id);
        }
    }, [runningAttempts, selectedAttempt]);

    const activeCount = runningAttempts.length;
    const selectedStatus = selectedAttempt
        ? runningAttempts.find((attempt) => attempt.id === selectedAttempt) ?? null
        : null;

    return (
        <div className="rounded-xl bg-card border border-border overflow-hidden shadow-sm flex flex-col h-[360px]">
            <div className="px-4 py-3 border-b border-border flex justify-between items-center gap-3">
                <div className="flex items-center gap-2 min-w-0">
                    <span className={`material-symbols-outlined text-sm ${activeCount > 0 ? 'text-success animate-pulse' : 'text-muted-foreground'}`}>
                        terminal
                    </span>
                    <h3 className="font-mono text-sm text-foreground font-bold">Agent Live Feed</h3>
                    {activeCount > 0 && (
                        <span className="text-[10px] bg-success/15 text-success px-1.5 py-0.5 rounded font-medium border border-success/30">
                            {activeCount} running
                        </span>
                    )}
                </div>
                <Link
                    to="/agent-logs"
                    className="text-xs text-muted-foreground hover:text-primary transition-colors flex items-center gap-1"
                >
                    View All
                    <span className="material-symbols-outlined text-[14px]">arrow_forward</span>
                </Link>
            </div>

            {error ? (
                <div className="flex-1 flex items-center justify-center px-6 text-center">
                    <div>
                        <span className="material-symbols-outlined text-red-500 text-3xl">error</span>
                        <p className="text-sm text-red-500 mt-2">Failed to load agent activity</p>
                        <p className="text-xs text-muted-foreground mt-1">{error}</p>
                    </div>
                </div>
            ) : loading ? (
                <div className="flex-1 flex items-center justify-center">
                    <div className="animate-spin rounded-full h-7 w-7 border-b-2 border-primary"></div>
                </div>
            ) : activeCount === 0 ? (
                <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground">
                    <span className="material-symbols-outlined text-4xl mb-2 text-muted-foreground/70">smart_toy</span>
                    <p className="text-sm font-medium">No active agents</p>
                    <p className="text-xs text-muted-foreground/80 mt-1">Start a task to see live timeline logs</p>
                </div>
            ) : (
                <>
                    <div className="px-3 py-2 border-b border-dashed border-border flex items-center gap-2 overflow-x-auto">
                        {runningAttempts.map((attempt) => {
                            const isSelected = attempt.id === selectedAttempt;
                            return (
                                <button
                                    key={attempt.id}
                                    onClick={() => setSelectedAttempt(attempt.id)}
                                    className={`shrink-0 px-2.5 py-1 rounded-md text-xs border transition-colors ${
                                        isSelected
                                            ? 'border-primary bg-primary/10 text-primary'
                                            : 'border-border text-muted-foreground hover:text-foreground hover:bg-muted/50'
                                    }`}
                                    title={`${attempt.project_name} - ${attempt.task_title}`}
                                >
                                    {attempt.task_title}
                                </button>
                            );
                        })}
                    </div>

                    <div className="flex-1 min-h-0">
                        {selectedAttempt && (
                            <TimelineLogDisplay
                                key={`dashboard-live-${selectedAttempt}`}
                                attemptId={selectedAttempt}
                                attemptStatus={selectedStatus?.status}
                            />
                        )}
                    </div>
                </>
            )}
        </div>
    );
}
