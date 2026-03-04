import { useState, useCallback } from 'react';
import {
  getPanelSizes,
  setPanelSizes,
} from '@/utils/panel-size-persistence';

/**
 * Hook to manage panel sizes with localStorage persistence
 */
export function usePanelSizes(key: string, defaults: number[]) {
  const [sizes, setSizes] = useState<number[]>(() => {
    const stored = getPanelSizes(key);
    return stored || defaults;
  });

  const updateSizes = useCallback(
    (newSizes: number[]) => {
      setSizes(newSizes);
      setPanelSizes(key, newSizes);
    },
    [key]
  );

  return [sizes, updateSizes] as const;
}
