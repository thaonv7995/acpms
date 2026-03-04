import { useState, FormEvent } from 'react';
import { Task, TaskType, CreateTaskRequest } from '../../shared/types';
import { createTask } from '../../api/tasks';
import { ApiError } from '../../api/client';

interface TaskCreateFormProps {
  projectId: string;
  onClose: () => void;
  onCreate: () => void;
  availableParents?: Task[];
}

const taskTypes: TaskType[] = [
  'feature',
  'bug',
  'hotfix',
  'refactor',
  'docs',
  'test',
  'chore',
  'spike',
  'small_task',
  'deploy',
];

export function TaskCreateForm({
  projectId,
  onClose,
  onCreate,
  availableParents = [],
}: TaskCreateFormProps) {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [taskType, setTaskType] = useState<TaskType>('feature');
  const [parentTaskId, setParentTaskId] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError('');

    try {
      const data: CreateTaskRequest = {
        project_id: projectId,
        title,
        description: description || undefined,
        task_type: taskType,
        parent_task_id: parentTaskId || undefined,
      };

      await createTask(data);
      onCreate();
      onClose();
    } catch (err) {
      if (err instanceof ApiError) {
        setError(err.message);
      } else {
        setError('Failed to create task');
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50">
      <div className="bg-white rounded-lg max-w-lg w-full">
        <div className="px-6 py-4 border-b border-gray-200 flex justify-between items-center">
          <h2 className="text-xl font-bold text-gray-900">Create New Task</h2>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600"
          >
            <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {error && (
            <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
              {error}
            </div>
          )}

          <div>
            <label htmlFor="title" className="block text-sm font-medium text-gray-700 mb-1">
              Title *
            </label>
            <input
              type="text"
              id="title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              required
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500"
              placeholder="Enter task title"
            />
          </div>

          <div>
            <label htmlFor="description" className="block text-sm font-medium text-gray-700 mb-1">
              Description
            </label>
            <textarea
              id="description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={3}
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500"
              placeholder="Enter task description"
            />
          </div>

          <div>
            <label htmlFor="taskType" className="block text-sm font-medium text-gray-700 mb-1">
              Type *
            </label>
            <select
              id="taskType"
              value={taskType}
              onChange={(e) => setTaskType(e.target.value as TaskType)}
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            >
              {taskTypes.map((type) => (
                <option key={type} value={type}>
                  {type.replace('_', ' ')}
                </option>
              ))}
            </select>
          </div>

          {availableParents.length > 0 && (
            <div>
              <label htmlFor="parentTaskId" className="block text-sm font-medium text-gray-700 mb-1">
                Parent Task (Optional)
              </label>
              <select
                id="parentTaskId"
                value={parentTaskId}
                onChange={(e) => setParentTaskId(e.target.value)}
                className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-blue-500 focus:border-blue-500"
              >
                <option value="">None</option>
                {availableParents.map((parent) => (
                  <option key={parent.id} value={parent.id}>
                    {parent.title} ({parent.task_type.replace('_', ' ')})
                  </option>
                ))}
              </select>
            </div>
          )}

          <div className="flex justify-end space-x-3 pt-4 border-t border-gray-200">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-gray-300 rounded-md text-gray-700 hover:bg-gray-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading}
              className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50"
            >
              {loading ? 'Creating...' : 'Create Task'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
