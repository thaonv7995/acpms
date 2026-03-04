import { useState, useEffect } from 'react';
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  DragStartEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import { Task, TaskStatus } from '../../shared/types';
import { KanbanColumn } from './KanbanColumn';
import { TaskCard } from './TaskCard';
import { updateTaskStatus } from '../../api/tasks';
import { ApiError } from '../../api/client';

interface KanbanBoardProps {
  tasks: Task[];
  onTasksChange: () => void;
  onTaskClick: (task: Task) => void;
}

const COLUMNS: TaskStatus[] = ['todo', 'in_progress', 'in_review', 'done'];

export function KanbanBoard({ tasks, onTasksChange, onTaskClick }: KanbanBoardProps) {
  const [activeTask, setActiveTask] = useState<Task | null>(null);
  const [localTasks, setLocalTasks] = useState<Task[]>(tasks);
  const [error, setError] = useState('');

  useEffect(() => {
    setLocalTasks(tasks);
  }, [tasks]);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    })
  );

  const handleDragStart = (event: DragStartEvent) => {
    const task = localTasks.find((t) => t.id === event.active.id);
    if (task) {
      setActiveTask(task);
    }
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveTask(null);

    if (!over || active.id === over.id) {
      return;
    }

    const taskId = active.id as string;
    const newStatus = over.id as TaskStatus;
    const task = localTasks.find((t) => t.id === taskId);

    if (!task || task.status === newStatus) {
      return;
    }

    // Optimistic update
    setLocalTasks((prevTasks) =>
      prevTasks.map((t) =>
        t.id === taskId ? { ...t, status: newStatus } : t
      )
    );

    try {
      await updateTaskStatus(taskId, newStatus);
      onTasksChange();
    } catch (err) {
      // Revert on error
      setLocalTasks(tasks);
      if (err instanceof ApiError) {
        setError(err.message);
      } else {
        setError('Failed to update task status');
      }
      setTimeout(() => setError(''), 3000);
    }
  };

  const getTasksByStatus = (status: TaskStatus) => {
    return localTasks.filter((task) => task.status === status);
  };

  return (
    <div>
      {error && (
        <div className="mb-4 bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          {error}
        </div>
      )}

      <DndContext
        sensors={sensors}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
      >
        <div className="flex gap-4 overflow-x-auto pb-4">
          {COLUMNS.map((status) => (
            <KanbanColumn
              key={status}
              status={status}
              tasks={getTasksByStatus(status)}
              onTaskClick={onTaskClick}
            />
          ))}
        </div>

        <DragOverlay>
          {activeTask ? (
            <div className="rotate-3">
              <TaskCard task={activeTask} onClick={() => { }} />
            </div>
          ) : null}
        </DragOverlay>
      </DndContext>
    </div>
  );
}
