import { describe, expect, it } from 'vitest';
import { getProjectProgressColor, normalizeProjectProgress } from '../../../utils/projectProgress';

describe('projectProgress utilities', () => {
  it('normalizes invalid or out-of-range values', () => {
    expect(normalizeProjectProgress(undefined)).toBe(0);
    expect(normalizeProjectProgress(Number.NaN)).toBe(0);
    expect(normalizeProjectProgress(-12)).toBe(0);
    expect(normalizeProjectProgress(42.4)).toBe(42);
    expect(normalizeProjectProgress(999)).toBe(100);
  });

  it('matches dashboard progress color thresholds', () => {
    expect(getProjectProgressColor(0)).toBe('bg-slate-400');
    expect(getProjectProgressColor(10)).toBe('bg-sky-500');
    expect(getProjectProgressColor(55)).toBe('bg-sky-500');
    expect(getProjectProgressColor(85)).toBe('bg-sky-500');
    expect(getProjectProgressColor(100)).toBe('bg-emerald-500');
  });
});
