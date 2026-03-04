import { useRef, useCallback, type ReactNode } from 'react';
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso';
import { useAutoScroll } from '@/hooks/useAutoScroll';

interface TimelineScrollContainerProps<T> {
  entries: T[];
  renderEntry: (entry: T, index: number) => ReactNode;
  autoScroll: boolean;
  hasOlderEntries?: boolean;
  isLoadingOlder?: boolean;
  onLoadOlder?: () => Promise<void> | void;
}

/**
 * Virtualized scroll container with timeline connection line.
 * Handles auto-scroll behavior and optimized rendering for large lists.
 */
export function TimelineScrollContainer<T>({
  entries,
  renderEntry,
  autoScroll,
  hasOlderEntries = false,
  isLoadingOlder = false,
  onLoadOlder,
}: TimelineScrollContainerProps<T>) {
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const { handleScroll, scrollToBottom, isNearBottom } = useAutoScroll(
    virtuosoRef,
    entries,
    { enabled: autoScroll, behavior: 'smooth' }
  );

  const handleAtBottomStateChange = useCallback(
    (atBottom: boolean) => {
      handleScroll(atBottom);
    },
    [handleScroll]
  );

  const handleStartReached = useCallback(() => {
    if (!hasOlderEntries || isLoadingOlder || !onLoadOlder) return;
    void onLoadOlder();
  }, [hasOlderEntries, isLoadingOlder, onLoadOlder]);

  return (
    <div className="relative flex-1 overflow-hidden bg-background">
      {/* Virtualized list */}
      <Virtuoso
        ref={virtuosoRef}
        data={entries}
        itemContent={(index, entry) => (
          <div>{renderEntry(entry, index)}</div>
        )}
        atBottomStateChange={handleAtBottomStateChange}
        startReached={handleStartReached}
        followOutput={autoScroll ? 'smooth' : false}
        initialTopMostItemIndex={entries.length > 0 ? entries.length - 1 : 0}
        overscan={200}
        className="h-full"
      />

      {/* Scroll to bottom button (shown when not at bottom) */}
      {!isNearBottom && (
        <button
          onClick={scrollToBottom}
          className="absolute bottom-4 right-4 z-20 h-8 px-3 inline-flex items-center bg-background text-muted-foreground border border-border rounded-sm hover:bg-muted/50 hover:text-foreground transition-colors"
          aria-label="Scroll to bottom"
        >
          <span className="flex items-center gap-2 text-xs font-medium">
            <svg
              className="w-3.5 h-3.5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M19 14l-7 7m0 0l-7-7m7 7V3"
              />
            </svg>
            Jump to bottom
          </span>
        </button>
      )}
    </div>
  );
}
