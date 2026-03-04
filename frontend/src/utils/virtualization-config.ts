/**
 * Virtualization configuration for log entry lists
 * Optimized for 10K+ entries with smooth scrolling
 */

export const VIRTUALIZATION_CONFIG = {
  /**
   * Estimated height of each log entry in pixels
   * Used to calculate virtual scroll height
   * Average entry: 60-100px depending on content
   */
  ITEM_SIZE_ESTIMATE: 80,

  /**
   * Number of items to render outside viewport
   * Higher = smoother scrolling but more memory
   * Range: 5-20, default: 10
   */
  OVERSCAN_COUNT: 10,

  /**
   * Additional vertical buffer beyond viewport (px)
   * Ensures smooth scrolling when user scrolls fast
   */
  INCREMENT_VIEWPORT_BY: 200,

  /**
   * Initial scroll position (0 = top, 'LAST' = bottom)
   */
  INITIAL_SCROLL: 'LAST' as const,

  /**
   * Scroll behavior ('auto' or 'smooth')
   * 'smooth' provides better UX but might lag on slow devices
   */
  SCROLL_BEHAVIOR: 'smooth' as const,

  /**
   * Threshold for showing "scroll to bottom" button (px from bottom)
   * If user scrolls up more than this, show button
   */
  AUTO_SCROLL_THRESHOLD: 100,

  /**
   * Animation duration for smooth scroll (ms)
   */
  SMOOTH_SCROLL_DURATION: 300,
};

/**
 * Get virtualization config variant for different scenarios
 */
export function getVirtualizationConfig(scenario: 'initial' | 'streaming' | 'search') {
  const base = VIRTUALIZATION_CONFIG;

  switch (scenario) {
    case 'initial':
      // Initial load: scrolls to bottom
      return {
        ...base,
        INITIAL_SCROLL: 'LAST' as const,
        OVERSCAN_COUNT: 5,
      };

    case 'streaming':
      // Live streaming: auto-scroll enabled, more overscan for smooth UX
      return {
        ...base,
        INITIAL_SCROLL: 'LAST' as const,
        OVERSCAN_COUNT: 15,
        INCREMENT_VIEWPORT_BY: 300,
      };

    case 'search':
      // Search results: scroll to first match
      return {
        ...base,
        INITIAL_SCROLL: 0,
        OVERSCAN_COUNT: 8,
      };

    default:
      return base;
  }
}

/**
 * Calculate ideal list height based on viewport
 */
export function getListHeight(): number {
  if (typeof window === 'undefined') return 600;

  // Full height minus header and footer (estimate)
  return Math.max(400, window.innerHeight - 200);
}

/**
 * Estimate total items height for scroll bar calculation
 */
export function estimateTotalHeight(itemCount: number, itemSize: number = VIRTUALIZATION_CONFIG.ITEM_SIZE_ESTIMATE): number {
  return itemCount * itemSize;
}
