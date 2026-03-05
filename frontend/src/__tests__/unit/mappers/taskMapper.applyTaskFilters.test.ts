import { describe, expect, it } from 'vitest';
import type { KanbanTask } from '../../../types/project';
import { applyTaskFilters } from '../../../mappers/taskMapper';

function makeTask(id: string, type: KanbanTask['type']): KanbanTask {
  return {
    id,
    title: `${type}-${id}`,
    type,
    status: 'todo',
    priority: 'medium',
    createdAt: '2026-03-01T00:00:00.000Z',
  };
}

describe('applyTaskFilters (Execution Only)', () => {
  it('keeps init tasks and filters out only docs/spike', () => {
    const tasks: KanbanTask[] = [
      makeTask('1', 'feature'),
      makeTask('2', 'init'),
      makeTask('3', 'docs'),
      makeTask('4', 'spike'),
    ];

    const filtered = applyTaskFilters(tasks, { agentOnly: true });

    expect(filtered.map((task) => task.type)).toEqual(['feature', 'init']);
  });
});

