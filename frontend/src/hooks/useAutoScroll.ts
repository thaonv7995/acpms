import { useEffect, useRef, useCallback, useState } from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';

interface UseAutoScrollOptions {
  enabled: boolean;
  threshold?: number;
  behavior?: 'auto' | 'smooth';
}

/**
 * Hook to manage auto-scroll behavior for virtualized lists
 *
 * Behavior:
 * 1. Auto-scrolls to bottom when new entries arrive (if enabled)
 * 2. Stops auto-scroll if user manually scrolls up (more than threshold)
 * 3. Resumes auto-scroll if user scrolls back to bottom
 * 4. Provides isNearBottom indicator for UI feedback
 */
export function useAutoScroll<T>(
  listRef: React.RefObject<VirtuosoHandle>,
  entries: T[],
  options: UseAutoScrollOptions = {
    enabled: true,
    threshold: 100,
    behavior: 'smooth',
  }
) {
  // Start from 0 so initial mount with preloaded/cached entries still scrolls to latest item.
  const prevLengthRef = useRef(0);
  const userScrolledUpRef = useRef(false);
  const isAutoScrollingRef = useRef(false);
  const [isNearBottom, setIsNearBottom] = useState(true);

  // Reset manual scroll lock when a new/empty stream is mounted.
  useEffect(() => {
    if (entries.length <= 1) {
      userScrolledUpRef.current = false;
      setIsNearBottom(true);
    }
  }, [entries.length]);

  /**
   * Auto-scroll when new entries arrive
   */
  useEffect(() => {
    if (!options.enabled || !listRef.current) return;

    const prevLength = prevLengthRef.current;
    const newEntries = entries.length > prevLength;
    const isInitialFill = prevLength === 0 && entries.length > 0;
    prevLengthRef.current = entries.length;

    // Only auto-scroll if:
    // 1. New entries added
    // 2. Auto-scroll enabled
    // 3. User hasn't scrolled up
    if (newEntries && (!userScrolledUpRef.current || isInitialFill)) {
      isAutoScrollingRef.current = true;

      // Use setTimeout to ensure DOM has updated
      setTimeout(() => {
        if (listRef.current) {
          listRef.current.scrollToIndex({
            index: entries.length - 1,
            align: 'end',
            behavior: isInitialFill ? 'auto' : (options.behavior || 'smooth'),
          });
        }
        isAutoScrollingRef.current = false;
      }, 0);
    }
  }, [entries.length, options.enabled, options.behavior, listRef]);

  /**
   * Track scroll position to determine if near bottom
   */
  const handleScroll = useCallback(
    (isAtBottom: boolean) => {
      setIsNearBottom(isAtBottom);

      // Only update if scroll wasn't triggered by auto-scroll
      if (!isAutoScrollingRef.current) {
        userScrolledUpRef.current = !isAtBottom;
      }
    },
    []
  );

  /**
   * Programmatically scroll to bottom
   * Called by ScrollToBottomButton
   */
  const scrollToBottom = useCallback(() => {
    if (listRef.current) {
      userScrolledUpRef.current = false;
      setIsNearBottom(true);
      listRef.current.scrollToIndex({
        index: entries.length - 1,
        align: 'end',
        behavior: options.behavior || 'smooth',
      });
    }
  }, [entries.length, options.behavior, listRef]);

  return {
    handleScroll,
    scrollToBottom,
    isNearBottom,
  };
}
