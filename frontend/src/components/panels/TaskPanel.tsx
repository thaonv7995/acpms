import { useState } from 'react';
import type { KanbanTask } from '../../types/project';
import { useGetTaskAttempts } from '../../api/generated/task-attempts/task-attempts';
import { NewCardContent } from '../ui/new-card';
import { Button } from '../ui/button';
import { Plus } from 'lucide-react';
import WYSIWYGEditor from '../ui/wysiwyg';
import { DataTable, type ColumnDef } from '../ui/table/data-table';
import type { TaskAttempt } from '../../types/task-attempt';
import { useElapsedRealtime } from '@/hooks/useElapsedRealtime';
import { formatElapsed } from '@/utils/elapsedTime';

interface TaskPanelProps {
  task: KanbanTask;
  projectId: string;
  onAttemptSelect: (attemptId: string) => void;
  onCreateAttempt: () => void;
}

/**
 * TaskPanel - Shows task details when no attempt is selected.
 * Matches vibe-kanban-reference implementation.
 */
// Helper to format time ago
function formatTimeAgo(dateString: string): string {
  const d = new Date(dateString);
  const diffMs = Date.now() - d.getTime();
  const absSec = Math.round(Math.abs(diffMs) / 1000);

  const rtf =
    typeof Intl !== 'undefined' &&
    typeof Intl.RelativeTimeFormat === 'function'
      ? new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })
      : null;

  const to = (value: number, unit: Intl.RelativeTimeFormatUnit) =>
    rtf
      ? rtf.format(-value, unit)
      : `${value} ${unit}${value !== 1 ? 's' : ''} ago`;

  if (absSec < 60) return to(Math.round(absSec), 'second');
  const mins = Math.round(absSec / 60);
  if (mins < 60) return to(mins, 'minute');
  const hours = Math.round(mins / 60);
  if (hours < 24) return to(hours, 'hour');
  const days = Math.round(hours / 24);
  if (days < 30) return to(days, 'day');
  const months = Math.round(days / 30);
  if (months < 12) return to(months, 'month');
  const years = Math.round(months / 12);
  return to(years, 'year');
}

function AttemptTimeCell({
  attempt,
  formatTimeAgo,
}: {
  attempt: TaskAttempt;
  formatTimeAgo: (date: string) => string;
}) {
  const isRunning = attempt.status === 'running';
  const elapsed = useElapsedRealtime(attempt.started_at ?? null, isRunning);
  if (isRunning && attempt.started_at && elapsed) {
    return <span title="Elapsed">Running {elapsed}</span>;
  }
  const end = attempt.ended_at ?? attempt.completed_at;
  if (
    (attempt.status === 'completed' || attempt.status === 'failed') &&
    attempt.started_at &&
    end
  ) {
    return (
      <span title="Duration">
        {formatElapsed(attempt.started_at, end)}
      </span>
    );
  }
  return <span>{formatTimeAgo(attempt.created_at)}</span>;
}

export function TaskPanel({ task, onAttemptSelect, onCreateAttempt }: TaskPanelProps) {
  const [isDescriptionExpanded, setIsDescriptionExpanded] = useState(false);

  const isAttemptActiveStatus = (status: string | undefined) => {
    if (!status) return false;
    const normalized = status.toLowerCase();
    return normalized === 'queued' || normalized === 'running';
  };

  const getAttemptsRefetchInterval = (query: { state: { data?: unknown } }) => {
    const attempts = (query.state.data as { data?: Array<{ status?: string }> } | undefined)?.data ?? [];
    return attempts.some((attempt) => isAttemptActiveStatus(attempt.status)) ? 5000 : false;
  };

  // Fetch attempts for this task
  const { data: attemptsResponse, isLoading: attemptsLoading } = useGetTaskAttempts(task.id, {
    query: {
      refetchInterval: getAttemptsRefetchInterval,
      refetchIntervalInBackground: false,
    },
  });

  const attempts = attemptsResponse?.data ?? [];
  // Sort by created_at DESC (most recent first) and map to TaskAttempt type
  const displayedAttempts: TaskAttempt[] = [...attempts]
    .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
    .map((dto) => ({
      id: dto.id,
      task_id: dto.task_id,
      status: dto.status.toLowerCase() as TaskAttempt['status'],
      started_at: dto.started_at ?? undefined,
      completed_at: dto.completed_at ?? undefined,
      ended_at: dto.completed_at ?? undefined,
      error_message: dto.error_message ?? undefined,
      created_at: dto.created_at,
      executor: (dto.metadata as { executor?: string })?.executor,
      branch: (dto.metadata as { branch?: string })?.branch,
    }));

  const titleContent = `# ${task.title || 'Task'}`;
  const descriptionContent = task.description || '';

  // Check if description needs truncation (more than ~2 lines of text)
  // Approximate: ~50-60 characters per line, so ~100-120 chars = ~2 lines
  const shouldTruncate = descriptionContent.length > 120;

  const attemptColumns: ColumnDef<TaskAttempt>[] = [
    {
      id: 'executor',
      header: '',
      accessor: (attempt) => attempt.executor || 'Base Agent',
      className: 'pr-4',
    },
    {
      id: 'branch',
      header: '',
      accessor: (attempt) => attempt.branch || '—',
      className: 'pr-4',
    },
    {
      id: 'time',
      header: '',
      accessor: (attempt) => (
        <AttemptTimeCell attempt={attempt} formatTimeAgo={formatTimeAgo} />
      ),
      className: 'pr-0 text-right',
    },
  ];

  return (
    <NewCardContent>
      <div className="p-6 flex flex-col h-full max-h-[calc(100vh-8rem)]">
        <div className="space-y-3 overflow-y-auto flex-shrink min-h-0">
          <WYSIWYGEditor value={titleContent} disabled className="text-sm" />
          {descriptionContent && (
            <div className="space-y-2">
              <div className={isDescriptionExpanded ? '' : 'line-clamp-2 overflow-hidden'}>
                <WYSIWYGEditor value={descriptionContent} disabled className="text-sm" />
              </div>
              {shouldTruncate && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setIsDescriptionExpanded(!isDescriptionExpanded);
                  }}
                  className="text-xs text-primary hover:text-primary/80 transition-colors"
                >
                  {isDescriptionExpanded ? 'Show less' : 'Show more'}
                </button>
              )}
            </div>
          )}
        </div>

        <div className="mt-6 flex-shrink-0 space-y-4">
          {attemptsLoading ? (
            <div className="text-muted-foreground">Loading attempts...</div>
          ) : displayedAttempts.length === 0 ? (
            <div className="text-muted-foreground">No attempts yet</div>
          ) : (
            <DataTable
              data={displayedAttempts}
              columns={attemptColumns}
              keyExtractor={(attempt) => attempt.id}
              onRowClick={(attempt) => onAttemptSelect(attempt.id)}
              emptyState="No attempts yet"
              headerContent={
                <div className="w-full flex text-left">
                  <span className="flex-1">
                    {displayedAttempts.length} attempt{displayedAttempts.length !== 1 ? 's' : ''}
                  </span>
                  <span>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={onCreateAttempt}
                    >
                      <Plus size={16} />
                    </Button>
                  </span>
                </div>
              }
            />
          )}
        </div>
      </div>
    </NewCardContent>
  );
}
