import { format } from 'date-fns';
import {
  Clock,
  CheckCircle,
  XCircle,
  Loader2,
  ChevronRight,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import type { TaskAttempt } from '@/types/task-attempt';

interface AttemptHistoryPanelProps {
  attempts: TaskAttempt[];
  currentAttemptId?: string;
  onSelectAttempt: (attemptId: string) => void;
}

export function AttemptHistoryPanel({
  attempts,
  currentAttemptId,
  onSelectAttempt,
}: AttemptHistoryPanelProps) {
  const sortedAttempts = [...attempts].sort(
    (a, b) =>
      new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
  );

  return (
    <div className="flex flex-col h-full bg-card">
      {/* Header */}
      <div className="p-3 border-b bg-card">
        <h3 className="font-medium text-sm text-foreground">Attempt History</h3>
        <p className="text-xs text-muted-foreground mt-1">
          {attempts.length} attempt{attempts.length !== 1 ? 's' : ''}
        </p>
      </div>

      {/* Attempts List */}
      <div className="flex-1 overflow-y-auto">
        {sortedAttempts.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm p-4 text-center">
            No attempts yet. Create one to get started.
          </div>
        ) : (
          <div className="divide-y divide-border">
            {sortedAttempts.map((attempt, index) => (
              <AttemptListItem
                key={attempt.id}
                attempt={attempt}
                attemptNumber={sortedAttempts.length - index}
                isSelected={attempt.id === currentAttemptId}
                onSelect={() => onSelectAttempt(attempt.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface AttemptListItemProps {
  attempt: TaskAttempt;
  attemptNumber: number;
  isSelected: boolean;
  onSelect: () => void;
}

function AttemptListItem({
  attempt,
  attemptNumber,
  isSelected,
  onSelect,
}: AttemptListItemProps) {
  const statusConfig = {
    pending: { icon: Clock, color: 'text-muted-foreground', label: 'Pending' },
    running: { icon: Loader2, color: 'text-blue-500', label: 'Running' },
    completed: { icon: CheckCircle, color: 'text-green-500', label: 'Completed' },
    failed: { icon: XCircle, color: 'text-red-500', label: 'Failed' },
    cancelled: { icon: XCircle, color: 'text-muted-foreground', label: 'Cancelled' },
  };

  const config = statusConfig[attempt.status as keyof typeof statusConfig] || {
    icon: Clock,
    color: 'text-muted-foreground',
    label: 'Unknown',
  };

  const Icon = config.icon;

  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full p-3 text-left transition-colors duration-200',
        'hover:bg-accent/50',
        isSelected && 'bg-accent border-l-2 border-l-primary'
      )}
    >
      <div className="flex items-start gap-3">
        {/* Status Icon */}
        <Icon
          className={cn(
            'w-4 h-4 mt-0.5 flex-shrink-0',
            config.color,
            attempt.status === 'running' && 'animate-spin'
          )}
          aria-label={`Status: ${config.label}`}
        />

        {/* Content */}
        <div className="flex-1 min-w-0">
          {/* Attempt Number + Selection Indicator */}
          <div className="flex items-center justify-between gap-2">
            <span className="font-medium text-sm text-foreground">
              Attempt #{attemptNumber}
            </span>
            {isSelected && (
              <ChevronRight className="w-4 h-4 text-primary flex-shrink-0" />
            )}
          </div>

          {/* Executor + Variant */}
          {attempt.executor && (
            <div className="text-xs text-muted-foreground mt-1">
              {attempt.executor}
              {attempt.variant && attempt.variant !== 'default' && (
                <span className="ml-1">({attempt.variant})</span>
              )}
            </div>
          )}

          {/* Timestamp */}
          <div className="text-xs text-muted-foreground mt-1">
            {attempt.started_at ? (
              <>
                Started{' '}
                <time dateTime={attempt.started_at}>
                  {format(new Date(attempt.started_at), 'MMM d, h:mm a')}
                </time>
              </>
            ) : (
              <>
                Created{' '}
                <time dateTime={attempt.created_at}>
                  {format(new Date(attempt.created_at), 'MMM d, h:mm a')}
                </time>
              </>
            )}
          </div>

          {/* Branch Info */}
          {attempt.branch && (
            <div className="text-xs text-muted-foreground mt-1 font-mono">
              {attempt.branch}
            </div>
          )}

          {/* Duration for Completed/Failed */}
          {attempt.status === 'completed' && attempt.started_at && attempt.ended_at && (
            <div className="text-xs text-green-600 dark:text-green-400 mt-1">
              Completed in {calculateDuration(attempt.started_at, attempt.ended_at)}
            </div>
          )}

          {attempt.status === 'failed' && attempt.started_at && attempt.ended_at && (
            <div className="text-xs text-red-600 dark:text-red-400 mt-1">
              Failed after {calculateDuration(attempt.started_at, attempt.ended_at)}
            </div>
          )}
        </div>
      </div>
    </button>
  );
}

function calculateDuration(start: string, end: string): string {
  const ms = new Date(end).getTime() - new Date(start).getTime();
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);

  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}
