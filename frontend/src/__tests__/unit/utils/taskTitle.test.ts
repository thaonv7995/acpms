import { describe, expect, it } from 'vitest';
import { getKanbanDisplayTitle } from '../../../utils/taskTitle';

describe('getKanbanDisplayTitle', () => {
  it('removes breakdown/ai prefix blocks from start of title', () => {
    expect(getKanbanDisplayTitle('[Breakdown][AI] Implement auth')).toBe('Implement auth');
    expect(getKanbanDisplayTitle('[Breakdown] [AI] Implement auth')).toBe('Implement auth');
    expect(getKanbanDisplayTitle('[AI][Breakdown] Implement auth')).toBe('Implement auth');
  });

  it('keeps regular titles unchanged', () => {
    expect(getKanbanDisplayTitle('Implement billing API')).toBe('Implement billing API');
  });

  it('falls back to original when title only contains prefixes', () => {
    expect(getKanbanDisplayTitle('[Breakdown][AI]')).toBe('[Breakdown][AI]');
  });
});

