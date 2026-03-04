import type { TaskAttempt } from '../../api/taskAttempts';
import { AttemptRetryInfo } from './AttemptRetryInfo';

interface TaskAttemptsSectionProps {
    attempts: TaskAttempt[];
    onRetryTriggered?: () => void;
}

export function TaskAttemptsSection({ attempts, onRetryTriggered }: TaskAttemptsSectionProps) {
    return (
        <div className="bg-white dark:bg-surface-dark rounded-xl border border-slate-200 dark:border-slate-700 p-6">
            <h3 className="text-sm font-bold text-slate-900 dark:text-white uppercase mb-4 flex items-center gap-2">
                <span className="material-symbols-outlined text-[18px] text-slate-500">history</span>
                Attempts ({attempts.length})
            </h3>
            {attempts.length === 0 ? (
                <div className="text-center py-8 text-slate-500 dark:text-slate-400">
                    <span className="material-symbols-outlined text-4xl mb-2 block">smart_toy</span>
                    <p>No attempts yet. Start an agent to create the first attempt.</p>
                </div>
            ) : (
                <div className="space-y-3">
                    {attempts.map((attempt) => (
                        <div
                            key={attempt.id}
                            className="p-4 rounded-lg bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 hover:border-primary/50 transition-colors cursor-pointer"
                        >
                            <div className="flex items-center justify-between mb-2">
                                <div className="flex items-center gap-2">
                                    <span className={`size-2 rounded-full ${
                                        attempt.status.toLowerCase() === 'success' ? 'bg-green-500' :
                                        attempt.status.toLowerCase() === 'failed' ? 'bg-red-500' :
                                        attempt.status.toLowerCase() === 'running' ? 'bg-blue-500 animate-pulse' :
                                        'bg-slate-400'
                                    }`}></span>
                                    <span className="text-sm font-medium text-slate-900 dark:text-white">
                                        {attempt.status}
                                    </span>
                                </div>
                                <span className="text-xs text-slate-500 dark:text-slate-400">
                                    {new Date(attempt.created_at).toLocaleString()}
                                </span>
                            </div>
                            {typeof attempt.metadata?.branch === 'string' && (
                                <div className="flex items-center gap-2 text-sm text-slate-600 dark:text-slate-400">
                                    <span className="material-symbols-outlined text-[16px]">commit</span>
                                    <span className="font-mono">{attempt.metadata.branch}</span>
                                </div>
                            )}
                            <AttemptRetryInfo
                                attemptId={attempt.id}
                                status={attempt.status}
                                onRetryTriggered={onRetryTriggered}
                            />
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}
