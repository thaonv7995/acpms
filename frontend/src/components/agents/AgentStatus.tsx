import { AttemptStatus } from '../../api/taskAttempts';

interface AgentStatusProps {
  status: AttemptStatus;
  startedAt?: string | null;
  completedAt?: string | null;
}

const statusConfig: Record<
  AttemptStatus,
  { label: string; color: string; bgColor: string; icon: string }
> = {
  QUEUED: {
    label: 'Queued',
    color: 'text-gray-700',
    bgColor: 'bg-gray-100',
    icon: '⏳',
  },
  RUNNING: {
    label: 'Running',
    color: 'text-blue-700',
    bgColor: 'bg-blue-100',
    icon: '▶️',
  },
  SUCCESS: {
    label: 'Success',
    color: 'text-green-700',
    bgColor: 'bg-green-100',
    icon: '✅',
  },
  FAILED: {
    label: 'Failed',
    color: 'text-red-700',
    bgColor: 'bg-red-100',
    icon: '❌',
  },
  CANCELLED: {
    label: 'Cancelled',
    color: 'text-yellow-700',
    bgColor: 'bg-yellow-100',
    icon: '⛔',
  },
};

export function AgentStatus({ status, startedAt, completedAt }: AgentStatusProps) {
  const config = statusConfig[status];

  const getDuration = () => {
    if (!startedAt) return null;
    const end = completedAt ? new Date(completedAt) : new Date();
    const start = new Date(startedAt);
    const durationMs = end.getTime() - start.getTime();
    const seconds = Math.floor(durationMs / 1000);
    const minutes = Math.floor(seconds / 60);

    if (minutes > 0) {
      return `${minutes}m ${seconds % 60}s`;
    }
    return `${seconds}s`;
  };

  const duration = getDuration();

  return (
    <div className="flex items-center gap-3">
      <div
        className={`flex items-center gap-2 px-3 py-1.5 rounded-full ${config.bgColor}`}
      >
        <span className="text-base">{config.icon}</span>
        <span className={`text-sm font-medium ${config.color}`}>
          {config.label}
        </span>
      </div>
      {duration && (
        <span className="text-sm text-gray-500">
          {status === 'RUNNING' ? 'Running for' : 'Duration'}: {duration}
        </span>
      )}
    </div>
  );
}
