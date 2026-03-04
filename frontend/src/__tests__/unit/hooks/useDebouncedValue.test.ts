import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useDebouncedValue } from '../../../hooks/useDebouncedValue';

describe('useDebouncedValue', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should return initial value immediately', () => {
    const { result } = renderHook(() => useDebouncedValue('test', 300));
    expect(result.current).toBe('test');
  });

  it('should debounce value changes', () => {
    const { result, rerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: 'initial', delay: 300 } }
    );

    // Initial value
    expect(result.current).toBe('initial');

    // Update value
    rerender({ value: 'updated', delay: 300 });

    // Value should not change immediately
    expect(result.current).toBe('initial');

    // Fast-forward time by 300ms
    vi.advanceTimersByTime(300);

    // Value should now be updated
    waitFor(() => {
      expect(result.current).toBe('updated');
    });
  });

  it('should reset timer on rapid changes', () => {
    const { result, rerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: 'initial', delay: 300 } }
    );

    // Update value multiple times rapidly
    rerender({ value: 'first', delay: 300 });
    vi.advanceTimersByTime(100);

    rerender({ value: 'second', delay: 300 });
    vi.advanceTimersByTime(100);

    rerender({ value: 'third', delay: 300 });
    vi.advanceTimersByTime(100);

    // Timer should be reset, value still initial
    expect(result.current).toBe('initial');

    // Fast-forward remaining time (200ms more needed for total 300ms from last change)
    vi.advanceTimersByTime(200);

    // Value should now be 'third'
    waitFor(() => {
      expect(result.current).toBe('third');
    });
  });

  it('should use custom delay', () => {
    const { result, rerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: 'initial', delay: 500 } }
    );

    rerender({ value: 'updated', delay: 500 });

    // After 300ms, value should still be initial
    vi.advanceTimersByTime(300);
    expect(result.current).toBe('initial');

    // After full 500ms, value should be updated
    vi.advanceTimersByTime(200);
    waitFor(() => {
      expect(result.current).toBe('updated');
    });
  });

  it('should handle different value types', () => {
    // Test with number
    const { result: numberResult, rerender: numberRerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: 0, delay: 300 } }
    );

    numberRerender({ value: 42, delay: 300 });
    vi.advanceTimersByTime(300);
    waitFor(() => {
      expect(numberResult.current).toBe(42);
    });

    // Test with object
    const { result: objectResult, rerender: objectRerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: { a: 1 }, delay: 300 } }
    );

    const newObj = { a: 2 };
    objectRerender({ value: newObj, delay: 300 });
    vi.advanceTimersByTime(300);
    waitFor(() => {
      expect(objectResult.current).toEqual(newObj);
    });
  });
});
