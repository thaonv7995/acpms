// TaskAttemptPanel - Shows attempt details with conversation-style logs
// Matches vibe-kanban-reference pattern with render prop
import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import type { KanbanTask } from '../../types/project';
import type { TaskAttempt } from '../../types/task-attempt';
import { TimelineLogDisplay } from '../timeline-log';
import { TaskFollowUpSection } from '../tasks-page/TaskFollowUpSection';
import { RetryUiProvider } from '../../contexts/RetryUiContext';
import { EntriesProvider } from '../../contexts/EntriesContext';
import { sendAttemptInput } from '@/api/taskAttempts';
import { useExecutionProcessesStream } from '@/hooks/useExecutionProcessesStream';
import type { TimelineTokenUsageInfo } from '@/types/timeline-log';

interface TaskAttemptPanelProps {
    task: KanbanTask;
    attempt: TaskAttempt | null;
    onAttemptStatusChange?: (status: string | null) => void;
    onFollowUpAttemptCreated?: (attemptId: string) => void;
    children: (sections: { logs: ReactNode; followUp: ReactNode; isRunning: boolean }) => ReactNode;
}

export function TaskAttemptPanel({
    attempt,
    task,
    onAttemptStatusChange,
    onFollowUpAttemptCreated,
    children,
}: TaskAttemptPanelProps) {
    const [liveAttemptStatus, setLiveAttemptStatus] = useState<string | null>(attempt?.status ?? null);
    const [tokenUsageInfo, setTokenUsageInfo] = useState<TimelineTokenUsageInfo | null>(null);
    const { processes } = useExecutionProcessesStream(attempt?.id);

    useEffect(() => {
        setLiveAttemptStatus(attempt?.status ?? null);
        setTokenUsageInfo(null);
    }, [attempt?.id, attempt?.status]);

    if (!attempt) {
        return <div className="p-6 text-muted-foreground">Loading attempt...</div>;
    }

    if (!task) {
        return <div className="p-6 text-muted-foreground">Loading task...</div>;
    }

    const handleStatusFromStream = (status: string | null) => {
        if (!status) return;
        const normalizedStatus = status.toLowerCase();
        if (liveAttemptStatus !== normalizedStatus) {
            setLiveAttemptStatus(normalizedStatus);
            onAttemptStatusChange?.(normalizedStatus);
        }
    };

    const isRunning = liveAttemptStatus === 'running' || liveAttemptStatus === 'queued';
    const latestProcessId = processes.length > 0 ? processes[processes.length - 1].id : null;

    const handleTimelineSend = async (message: string) => {
        await sendAttemptInput(attempt.id, message);
    };

    const handleTimelineMetaSnapshot = (snapshot: {
        attemptStatus: string | null;
        tokenUsageInfo: TimelineTokenUsageInfo | null;
    }) => {
        setTokenUsageInfo(prev => {
            // Only update if actual values changed
            if (prev?.totalTokens === snapshot.tokenUsageInfo?.totalTokens &&
                prev?.inputTokens === snapshot.tokenUsageInfo?.inputTokens &&
                prev?.outputTokens === snapshot.tokenUsageInfo?.outputTokens) {
                return prev;
            }
            return snapshot.tokenUsageInfo;
        });
    };

    return (
        <EntriesProvider key={attempt.id}>
            <RetryUiProvider>
                {children({
                    logs: (
                        <TimelineLogDisplay
                            key={attempt.id}
                            attemptId={attempt.id}
                            executionProcesses={processes}
                            enableChat={isRunning}
                            onSendMessage={handleTimelineSend}
                            attemptStatus={liveAttemptStatus ?? attempt.status}
                            onAttemptStatusChange={handleStatusFromStream}
                            onMetaSnapshotChange={handleTimelineMetaSnapshot}
                            showStatusInHeader={isRunning}
                            showTokenUsageInHeader={isRunning}
                        />
                    ),
                    followUp: (
                        <TaskFollowUpSection
                            sessionId={attempt.id}
                            isRunning={isRunning}
                            disabled={false}
                            retryProcessId={latestProcessId}
                            onFollowUpAttemptCreated={onFollowUpAttemptCreated}
                            taskId={task.id}
                            projectId={task.projectId ?? null}
                            attemptStatus={liveAttemptStatus}
                            attemptErrorMessage={attempt.error_message}
                            tokenUsageInfo={tokenUsageInfo}
                        />
                    ),
                    isRunning,
                })}
            </RetryUiProvider>
        </EntriesProvider>
    );
}
