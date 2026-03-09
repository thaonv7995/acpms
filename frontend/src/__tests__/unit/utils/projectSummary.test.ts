import { describe, expect, it } from 'vitest';
import { getProjectStatusPresentation, normalizeProjectLifecycleStatus } from '../../../utils/projectSummary';

describe('projectSummary utilities', () => {
  it('prefers backend summary over legacy metadata', () => {
    const presentation = getProjectStatusPresentation({
      lifecycle_status: 'blocked',
      execution_status: 'failed',
      progress: 42,
      total_tasks: 12,
      completed_tasks: 5,
      active_tasks: 1,
      review_tasks: 0,
      blocked_tasks: 2,
    });

    expect(presentation).toEqual({
      status: 'blocked',
      statusLabel: 'Blocked',
      statusColor: 'red',
      progress: 42,
      agentCount: 1,
    });
  });

  it('accepts current lifecycle statuses', () => {
    expect(normalizeProjectLifecycleStatus('planning')).toBe('planning');
    expect(normalizeProjectLifecycleStatus('active')).toBe('active');
    expect(normalizeProjectLifecycleStatus('reviewing')).toBe('reviewing');
    expect(normalizeProjectLifecycleStatus('paused')).toBe('paused');
  });

  it('falls back to planning when no summary or legacy metadata exists', () => {
    expect(getProjectStatusPresentation()).toEqual({
      status: 'planning',
      statusLabel: 'Planning',
      statusColor: 'slate',
      progress: 0,
      agentCount: 0,
    });
  });

  it('ignores legacy metadata status and progress when backend summary is missing', () => {
    expect(getProjectStatusPresentation()).toEqual({
      status: 'planning',
      statusLabel: 'Planning',
      statusColor: 'slate',
      progress: 0,
      agentCount: 0,
    });
  });
});
