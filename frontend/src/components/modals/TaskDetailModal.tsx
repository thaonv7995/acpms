import type { KanbanTask } from '../../types/project';

interface TaskDetailModalProps {
  isOpen: boolean;
  onClose: () => void;
  task: KanbanTask;
  projectId: string;
  onEdit?: () => void;
  previewUrl?: string;
}

const statusLabels: Record<string, string> = {
  backlog: 'Backlog',
  todo: 'To Do',
  in_progress: 'In Progress',
  in_review: 'In Review',
  done: 'Done',
};

const statusColors: Record<string, string> = {
  backlog: 'bg-muted-foreground/60',
  todo: 'bg-muted-foreground/60',
  in_progress: 'bg-blue-500',
  in_review: 'bg-amber-500',
  done: 'bg-emerald-500',
};

function formatType(type: string): string {
  return (type || 'feature').replace('_', ' ');
}

export function TaskDetailModal({
  isOpen,
  onClose,
  task,
  projectId: _projectId,
  onEdit,
  previewUrl,
}: TaskDetailModalProps) {
  if (!isOpen) return null;

  const displayStatus = statusLabels[task.status] || task.status;
  const statusColor = statusColors[task.status] || statusColors.todo;
  const canEdit = task.status === 'backlog' || task.status === 'todo' || task.status === 'in_review';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px] transition-opacity"
        onClick={onClose}
      />
      <div className="relative w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header - same style as EditTaskModal */}
        <div className="px-6 py-5 border-b border-border flex justify-between items-center bg-muted">
          <div>
            <h2 className="text-lg font-bold text-card-foreground">Task Details</h2>
            <p className="text-sm text-muted-foreground">View only</p>
          </div>
          <div className="flex items-center gap-2">
            {previewUrl && (
              <a
                href={previewUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 px-3 py-1.5 bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 rounded-lg transition-colors border border-emerald-500/20"
                title="Open Preview"
              >
                <span className="material-symbols-outlined text-[18px]">open_in_new</span>
                <span className="text-xs font-medium">Preview</span>
              </a>
            )}
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-card-foreground transition-colors"
            >
              <span className="material-symbols-outlined">close</span>
            </button>
          </div>
        </div>

        {/* Content - read-only */}
        <div className="p-6 overflow-y-auto flex flex-col gap-5">
          <div>
            <h3 className="text-sm font-bold text-card-foreground mb-1.5">Title</h3>
            <p className="text-sm text-card-foreground">{task.title || 'Untitled Task'}</p>
          </div>

          {task.description && (
            <div>
              <h3 className="text-sm font-bold text-card-foreground mb-1.5">Description</h3>
              <p className="text-sm text-muted-foreground whitespace-pre-wrap">{task.description}</p>
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <h3 className="text-xs font-bold text-muted-foreground uppercase mb-1.5">Status</h3>
              <div className="flex items-center gap-2">
                <span className={`size-2 rounded-full ${statusColor}`} />
                <span className="text-sm text-card-foreground">{displayStatus}</span>
              </div>
            </div>
            <div>
              <h3 className="text-xs font-bold text-muted-foreground uppercase mb-1.5">Type</h3>
              <span className="text-sm text-card-foreground capitalize">{formatType(task.type || 'feature')}</span>
            </div>
            <div>
              <h3 className="text-xs font-bold text-muted-foreground uppercase mb-1.5">Priority</h3>
              <span className="text-sm text-card-foreground capitalize">{task.priority || 'medium'}</span>
            </div>
            {task.createdAt && (
              <div>
                <h3 className="text-xs font-bold text-muted-foreground uppercase mb-1.5">Created</h3>
                <span className="text-sm text-muted-foreground">
                  {new Date(task.createdAt).toLocaleDateString()}
                </span>
              </div>
            )}
          </div>
        </div>

        {/* Footer - same style as EditTaskModal */}
        <div className="px-6 py-4 border-t border-border bg-muted flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
          >
            Close
          </button>
          {onEdit && canEdit && (
            <button
              onClick={() => {
                onClose();
                onEdit();
              }}
              className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95"
            >
              <span className="material-symbols-outlined text-[18px]">edit</span>
              Edit Task
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
