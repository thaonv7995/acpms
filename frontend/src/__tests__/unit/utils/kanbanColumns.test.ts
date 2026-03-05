import { describe, expect, it } from 'vitest';
import {
  createKanbanColumns,
  resolveKanbanColumnId,
  resolveKanbanColumnStatus,
} from '../../../utils/kanbanColumns';

describe('kanbanColumns config defaults', () => {
  it('shows todo/in-progress/in-review/done and hides closed by default', () => {
    const columns = createKanbanColumns();
    expect(columns.map((column) => column.status)).toEqual(['todo', 'in_progress', 'in_review', 'done']);
  });

  it('maps blocked status into in_review column', () => {
    expect(resolveKanbanColumnStatus('blocked')).toBe('in_review');
  });

  it('keeps backlog tasks visible under TODO when backlog column is disabled', () => {
    expect(resolveKanbanColumnStatus('todo')).toBe('todo');
    expect(resolveKanbanColumnStatus('backlog')).toBe('todo');
    expect(resolveKanbanColumnStatus('archived')).toBeNull();
  });

  it('keeps todo/backlog as distinct statuses when backlog column is enabled', () => {
    expect(
      resolveKanbanColumnId(
        { status: 'todo' },
        { showBacklog: true, showClosed: false }
      )
    ).toBe('col-todo');
    expect(
      resolveKanbanColumnId(
        { status: 'backlog' },
        { showBacklog: true, showClosed: false }
      )
    ).toBe('col-backlog');
  });
});
