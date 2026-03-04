import { createContext, useContext, useState, ReactNode } from 'react';

export interface RetryUiContextValue {
  retryProcessId: string | null;
  isRetrying: boolean;
  setRetryProcessId: (id: string | null) => void;
  setIsRetrying: (value: boolean) => void;
}

const RetryUiContext = createContext<RetryUiContextValue | undefined>(undefined);

export interface RetryUiProviderProps {
  children: ReactNode;
}

export function RetryUiProvider({ children }: RetryUiProviderProps) {
  const [retryProcessId, setRetryProcessId] = useState<string | null>(null);
  const [isRetrying, setIsRetrying] = useState(false);

  const value: RetryUiContextValue = {
    retryProcessId,
    isRetrying,
    setRetryProcessId,
    setIsRetrying,
  };

  return (
    <RetryUiContext.Provider value={value}>
      {children}
    </RetryUiContext.Provider>
  );
}

export function useRetryUi(): RetryUiContextValue {
  const context = useContext(RetryUiContext);
  if (!context) {
    throw new Error('useRetryUi must be used within RetryUiProvider');
  }
  return context;
}
