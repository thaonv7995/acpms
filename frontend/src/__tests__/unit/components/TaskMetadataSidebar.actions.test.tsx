import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TaskMetadataSidebar } from '@/components/task-detail-page/TaskMetadataSidebar';

describe('TaskMetadataSidebar actions', () => {
  it('wires edit and delete buttons when handlers are provided', () => {
    const onEditTask = vi.fn();
    const onDeleteTask = vi.fn();

    render(
      <TaskMetadataSidebar
        taskId="task-1"
        status="todo"
        priority="medium"
        type="feature"
        createdAt="2026-03-10T00:00:00.000Z"
        onEditTask={onEditTask}
        onDeleteTask={onDeleteTask}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Edit Task' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete Task' }));

    expect(onEditTask).toHaveBeenCalledTimes(1);
    expect(onDeleteTask).toHaveBeenCalledTimes(1);
  });

  it('hides edit action for non-editable statuses but keeps delete', () => {
    render(
      <TaskMetadataSidebar
        taskId="task-1"
        status="in_progress"
        priority="medium"
        type="feature"
        createdAt="2026-03-10T00:00:00.000Z"
        onEditTask={vi.fn()}
        onDeleteTask={vi.fn()}
      />
    );

    expect(screen.queryByRole('button', { name: 'Edit Task' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Delete Task' })).toBeTruthy();
  });
});
