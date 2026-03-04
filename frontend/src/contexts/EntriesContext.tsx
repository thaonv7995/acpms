import {
  createContext,
  useContext,
  useState,
  useMemo,
  useCallback,
  ReactNode,
} from 'react';
import type { PatchTypeWithKey } from '@/hooks/useConversationHistory';

interface EntriesContextType {
  entries: PatchTypeWithKey[];
  setEntries: (entries: PatchTypeWithKey[]) => void;
  reset: () => void;
}

const EntriesContext = createContext<EntriesContextType | null>(null);

interface EntriesProviderProps {
  children: ReactNode;
}

export function EntriesProvider({ children }: EntriesProviderProps) {
  const [entries, setEntriesState] = useState<PatchTypeWithKey[]>([]);

  const setEntries = useCallback((newEntries: PatchTypeWithKey[]) => {
    setEntriesState(newEntries);
  }, []);

  const reset = useCallback(() => {
    setEntriesState([]);
  }, []);

  const value = useMemo(
    () => ({
      entries,
      setEntries,
      reset,
    }),
    [entries, setEntries, reset]
  );

  return (
    <EntriesContext.Provider value={value}>
      {children}
    </EntriesContext.Provider>
  );
}

export function useEntries(): EntriesContextType {
  const context = useContext(EntriesContext);
  if (!context) {
    throw new Error('useEntries must be used within an EntriesProvider');
  }
  return context;
}
