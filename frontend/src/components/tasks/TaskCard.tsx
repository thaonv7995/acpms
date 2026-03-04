import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Task } from '../../shared/types';

interface TaskCardProps {
  task: Task;
  onClick: () => void;
}

export function TaskCard({ task, onClick }: TaskCardProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: task.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  const getTaskTypeColor = (type: string) => {
    switch (type) {
      case 'REQUIREMENT':
        return 'bg-blue-100 text-blue-800';
      case 'TASK':
        return 'bg-green-100 text-green-800';
      case 'SUBTASK':
        return 'bg-gray-100 text-gray-800';
      case 'BUG':
        return 'bg-red-100 text-red-800';
      case 'TEST_PLAN':
        return 'bg-purple-100 text-purple-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      onClick={onClick}
      className="bg-white rounded-lg shadow border border-gray-200 p-4 mb-3 cursor-pointer hover:shadow-md transition-shadow"
    >
      <div className="flex items-start justify-between mb-2">
        <h4 className="text-sm font-medium text-gray-900 line-clamp-2 flex-1">
          {task.title}
        </h4>
        <span className={`ml-2 px-2 py-1 text-xs font-medium rounded ${getTaskTypeColor(task.task_type)}`}>
          {task.task_type.replace('_', ' ')}
        </span>
      </div>

      {task.description && (
        <p className="text-xs text-gray-600 line-clamp-2 mb-2">{task.description}</p>
      )}

      {task.assigned_to && (
        <div className="flex items-center text-xs text-gray-500">
          <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
          </svg>
          Assigned
        </div>
      )}
    </div>
  );
}
