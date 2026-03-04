import { act, renderHook } from '@testing-library/react';
import type { RefObject } from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAutoScroll } from '../../../hooks/useAutoScroll';

function createListRef() {
  const scrollToIndex = vi.fn();
  const ref = {
    current: {
      scrollToIndex,
    } as unknown as VirtuosoHandle,
  } as RefObject<VirtuosoHandle>;

  return { ref, scrollToIndex };
}

describe('useAutoScroll', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('scrolls to latest item on initial mount when entries are already loaded', () => {
    const { ref, scrollToIndex } = createListRef();
    const entries = ['one', 'two', 'three'];

    renderHook(() =>
      useAutoScroll(ref, entries, {
        enabled: true,
        behavior: 'smooth',
      })
    );

    act(() => {
      vi.runAllTimers();
    });

    expect(scrollToIndex).toHaveBeenCalledWith({
      index: 2,
      align: 'end',
      behavior: 'auto',
    });
  });

  it('does not force auto-scroll when user has manually scrolled up', () => {
    const { ref, scrollToIndex } = createListRef();

    const { result, rerender } = renderHook(
      ({ entries }) =>
        useAutoScroll(ref, entries, {
          enabled: true,
          behavior: 'smooth',
        }),
      {
        initialProps: {
          entries: ['a', 'b'],
        },
      }
    );

    act(() => {
      vi.runAllTimers();
    });

    expect(scrollToIndex).toHaveBeenCalledTimes(1);

    act(() => {
      result.current.handleScroll(false);
    });

    rerender({
      entries: ['a', 'b', 'c'],
    });

    act(() => {
      vi.runAllTimers();
    });

    expect(scrollToIndex).toHaveBeenCalledTimes(1);
  });
});
