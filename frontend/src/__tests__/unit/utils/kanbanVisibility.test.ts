import { describe, expect, it } from 'vitest';
import { isBreakdownSupportTask } from '../../../utils/kanbanVisibility';

describe('isBreakdownSupportTask', () => {
  it('detects support tasks by title prefix', () => {
    expect(isBreakdownSupportTask({ title: '[Breakdown][AI] Analyze requirement' })).toBe(true);
    expect(isBreakdownSupportTask({ title: '[breakdown] Analyze requirement' })).toBe(true);
  });

  it('detects support tasks by metadata mode/kind', () => {
    expect(
      isBreakdownSupportTask({
        title: 'Analysis session',
        metadata: { breakdown_mode: 'ai_support' },
      })
    ).toBe(true);

    expect(
      isBreakdownSupportTask({
        title: 'Analysis session',
        metadata: { breakdown_kind: 'analysis_session' },
      })
    ).toBe(true);
  });

  it('does not hide normal implementation tasks', () => {
    expect(
      isBreakdownSupportTask({
        title: 'Implement login page',
        metadata: { breakdown_kind: 'implementation' },
      })
    ).toBe(false);
  });
});

