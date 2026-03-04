import { logger } from '@/lib/logger';
/**
 * Utility functions for persisting panel sizes to localStorage
 */

const STORAGE_KEY = 'panel-sizes';

export interface PanelSizes {
  'main-split'?: number[];
  'attempt-aux-split'?: number[];
  'diffs-aux-split'?: number[];
  'last-mode'?: string;
}

/**
 * Get panel sizes from localStorage
 */
export function getPanelSizes(key: string): number[] | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return null;

    const sizes: PanelSizes = JSON.parse(stored);
    return sizes[key as keyof PanelSizes] as number[] | null;
  } catch (error) {
    logger.error('Failed to get panel sizes:', error);
    return null;
  }
}

/**
 * Save panel sizes to localStorage
 */
export function setPanelSizes(key: string, sizes: number[]): void {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    const current: PanelSizes = stored ? JSON.parse(stored) : {};

    current[key as keyof PanelSizes] = sizes as any;

    localStorage.setItem(STORAGE_KEY, JSON.stringify(current));
  } catch (error) {
    logger.error('Failed to set panel sizes:', error);
  }
}

/**
 * Initialize panel sizes with defaults or restore from storage
 */
export function initializePanelSizes(defaults: PanelSizes): PanelSizes {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (!stored) return defaults;

    const sizes: PanelSizes = JSON.parse(stored);
    return { ...defaults, ...sizes };
  } catch (error) {
    logger.error('Failed to initialize panel sizes:', error);
    return defaults;
  }
}

/**
 * Clear all panel sizes from storage
 */
export function clearPanelSizes(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch (error) {
    logger.error('Failed to clear panel sizes:', error);
  }
}
