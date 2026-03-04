import { useDroppable } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { Task, TaskStatus } from '../../shared/types';
import { TaskCard } from './TaskCard';

interface KanbanColumnProps {
  status: TaskStatus;
  tasks: Task[];
  onTaskClick: (task: Task) => void;
}

const statusLabels: Record<TaskStatus, string> = {
  todo: 'To Do',
  in_progress: 'In Progress',
  in_review: 'Review',
  done: 'Done',
  blocked: 'Blocked',
  archived: 'Archived',
};

const statusColors: Record<TaskStatus, string> = {
  todo: 'bg-gray-100',
  in_progress: 'bg-blue-100',
  in_review: 'bg-yellow-100',
  done: 'bg-green-100',
  blocked: 'bg-red-100',
  archived: 'bg-gray-200',
};

export function KanbanColumn({ status, tasks, onTaskClick }: KanbanColumnProps) {
  const { setNodeRef, isOver } = useDroppable({
    id: status,
  });

  const taskIds = tasks.map(task => task.id);

  return (
    <div className="flex-1 min-w-[280px]">
      <div className={`rounded-t-lg px-4 py-2 ${statusColors[status]}`}>
        <h3 className="font-semibold text-gray-900 text-sm">
          {statusLabels[status]} ({tasks.length})
        </h3>
      </div>
      <div
        ref={setNodeRef}
        className={`bg-gray-50 rounded-b-lg p-4 min-h-[400px] transition-colors ${isOver ? 'bg-blue-50 border-2 border-blue-300 border-dashed' : 'border border-gray-200'
          }`}
      >
        <SortableContext items={taskIds} strategy={verticalListSortingStrategy}>
          {tasks.map((task) => (
            <TaskCard key={task.id} task={task} onClick={() => onTaskClick(task)} />
          ))}
        </SortableContext>
      </div>
    </div>
  );
}
