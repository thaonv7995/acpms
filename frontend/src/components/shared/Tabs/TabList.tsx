import { useTabsContext } from './Tabs';

export interface Tab {
  id: string;
  label: string;
  icon?: string;
}

interface TabListProps {
  tabs: Tab[];
}

export function TabList({ tabs }: TabListProps) {
  const { activeTab, setActiveTab } = useTabsContext();

  return (
    <div className="flex gap-1 border-b border-slate-200 dark:border-slate-700">
      {tabs.map((tab) => {
        const isActive = activeTab === tab.id;
        return (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`
              flex items-center gap-2 px-4 py-2.5 font-medium text-sm rounded-t-lg transition-colors
              ${
                isActive
                  ? 'text-primary border-b-2 border-primary bg-primary/5'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white hover:bg-slate-50 dark:hover:bg-slate-800'
              }
            `}
            role="tab"
            aria-selected={isActive}
            aria-controls={`tabpanel-${tab.id}`}
          >
            {tab.icon && (
              <span className="material-symbols-outlined text-base">{tab.icon}</span>
            )}
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
