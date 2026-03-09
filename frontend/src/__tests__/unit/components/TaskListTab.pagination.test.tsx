import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import { TaskListTab } from '@/components/project-detail/TaskListTab';
import type { KanbanTask } from '@/types/project';

function makeTask(index: number): KanbanTask {
  return {
    id: `task-${index}`,
    title: `Task ${index}`,
    type: 'feature',
    status: 'todo',
    priority: 'medium',
    createdAt: `2026-03-10T00:00:${String(index).padStart(2, '0')}.000Z`,
  };
}

function renderTaskList(
  tasks: KanbanTask[],
  onPaginationVisibilityChange = vi.fn()
) {
  return {
    onPaginationVisibilityChange,
    ...render(
      <MemoryRouter>
        <TaskListTab
          tasks={tasks}
          requirements={[]}
          projectId="project-1"
          sprints={[]}
          selectedSprintId={null}
          onSelectSprint={vi.fn()}
          onRefreshProject={vi.fn()}
          onTaskClick={vi.fn()}
          onViewLogs={vi.fn()}
          onPaginationVisibilityChange={onPaginationVisibilityChange}
        />
      </MemoryRouter>
    ),
  };
}

describe('TaskListTab pagination visibility', () => {
  it('reports when pagination controls are visible', async () => {
    const { onPaginationVisibilityChange } = renderTaskList(
      Array.from({ length: 11 }, (_, index) => makeTask(index + 1))
    );

    await waitFor(() => {
      expect(onPaginationVisibilityChange).toHaveBeenLastCalledWith(true);
    });
  });

  it('reports when pagination controls are hidden again', async () => {
    const onPaginationVisibilityChange = vi.fn();
    const { rerender } = renderTaskList(
      Array.from({ length: 2 }, (_, index) => makeTask(index + 1)),
      onPaginationVisibilityChange
    );

    await waitFor(() => {
      expect(onPaginationVisibilityChange).toHaveBeenLastCalledWith(true);
    });

    rerender(
      <MemoryRouter>
        <TaskListTab
          tasks={[]}
          requirements={[]}
          projectId="project-1"
          sprints={[]}
          selectedSprintId={null}
          onSelectSprint={vi.fn()}
          onRefreshProject={vi.fn()}
          onTaskClick={vi.fn()}
          onViewLogs={vi.fn()}
          onPaginationVisibilityChange={onPaginationVisibilityChange}
        />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(onPaginationVisibilityChange).toHaveBeenLastCalledWith(false);
    });
  });

  it('renders edit and delete actions and calls their handlers', () => {
    const onEditTask = vi.fn();
    const onDeleteTask = vi.fn();

    render(
      <MemoryRouter>
        <TaskListTab
          tasks={[makeTask(1)]}
          requirements={[]}
          projectId="project-1"
          sprints={[]}
          selectedSprintId={null}
          onSelectSprint={vi.fn()}
          onRefreshProject={vi.fn()}
          onTaskClick={vi.fn()}
          onViewLogs={vi.fn()}
          onEditTask={onEditTask}
          onDeleteTask={onDeleteTask}
        />
      </MemoryRouter>
    );

    fireEvent.click(screen.getByLabelText('Edit Task 1'));
    fireEvent.click(screen.getByLabelText('Delete Task 1'));

    expect(onEditTask).toHaveBeenCalledWith('task-1');
    expect(onDeleteTask).toHaveBeenCalledWith('task-1');
  });

  it('renders an icon-only logs action for completed tasks without a placeholder dash', () => {
    const onViewLogs = vi.fn();

    render(
      <MemoryRouter>
        <TaskListTab
          tasks={[{ ...makeTask(1), status: 'done' }]}
          requirements={[]}
          projectId="project-1"
          sprints={[]}
          selectedSprintId={null}
          onSelectSprint={vi.fn()}
          onRefreshProject={vi.fn()}
          onTaskClick={vi.fn()}
          onViewLogs={onViewLogs}
        />
      </MemoryRouter>
    );

    fireEvent.click(screen.getByRole('button', { name: 'Open logs for Task 1' }));

    expect(onViewLogs).toHaveBeenCalledWith('task-1');
    expect(screen.queryByText('—')).toBeNull();
  });
});
