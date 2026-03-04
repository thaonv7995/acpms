import { ReactNode } from 'react';
import { useTabsContext } from './Tabs';

interface TabPanelProps {
  id: string;
  children: ReactNode;
}

export function TabPanel({ id, children }: TabPanelProps) {
  const { activeTab } = useTabsContext();

  if (activeTab !== id) return null;

  return (
    <div
      role="tabpanel"
      id={`tabpanel-${id}`}
      aria-labelledby={`tab-${id}`}
      className="py-4"
    >
      {children}
    </div>
  );
}
