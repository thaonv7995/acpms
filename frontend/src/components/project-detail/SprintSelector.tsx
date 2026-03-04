import { useState, useRef, useEffect } from 'react';
import type { SprintDto } from '@/api/generated/models';

interface SprintSelectorProps {
  sprints: SprintDto[];
  selectedSprintId: string | null;
  onSelectSprint: (sprintId: string | null) => void;
  loading?: boolean;
}

const statusColors: Record<string, { bg: string; text: string; dot: string }> = {
  active: { bg: 'bg-green-100 dark:bg-green-500/20', text: 'text-green-600 dark:text-green-400', dot: 'bg-green-500' },
  planned: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', dot: 'bg-blue-500' },
  planning: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', dot: 'bg-blue-500' },
  closed: { bg: 'bg-muted', text: 'text-muted-foreground', dot: 'bg-muted-foreground/50' },
  completed: { bg: 'bg-muted', text: 'text-muted-foreground', dot: 'bg-muted-foreground/50' },
  archived: { bg: 'bg-muted', text: 'text-muted-foreground', dot: 'bg-muted-foreground/30' },
};

function normalizeSprintStatus(status: string): string {
  const lower = status.toLowerCase();
  if (lower === 'planning') return 'planned';
  if (lower === 'completed') return 'closed';
  return lower;
}

function formatSprintDates(sprint: SprintDto): string {
  if (!sprint.start_date) return '';
  const start = new Date(sprint.start_date);
  const end = sprint.end_date ? new Date(sprint.end_date) : null;

  const formatDate = (d: Date) => d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });

  if (end) {
    return `${formatDate(start)} - ${formatDate(end)}`;
  }
  return `From ${formatDate(start)}`;
}

export function SprintSelector({ sprints, selectedSprintId, onSelectSprint, loading }: SprintSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const selectedSprint = sprints.find(s => s.id === selectedSprintId);
  const activeSprint = sprints.find(s => normalizeSprintStatus(s.status) === 'active');

  // Sort sprints: active first, then by created_at desc
  const sortedSprints = [...sprints].sort((a, b) => {
    const statusA = normalizeSprintStatus(a.status);
    const statusB = normalizeSprintStatus(b.status);
    if (statusA === 'active' && statusB !== 'active') return -1;
    if (statusB === 'active' && statusA !== 'active') return 1;
    return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
  });

  const displaySprint = selectedSprint || activeSprint;
  const displayStatus = displaySprint ? normalizeSprintStatus(displaySprint.status) : 'planned';
  const statusStyle = statusColors[displayStatus] || statusColors.planned;

  if (loading) {
    return (
      <div className="animate-pulse flex items-center gap-2 px-3 py-2 bg-muted rounded-lg">
        <div className="h-4 w-20 bg-muted/50 rounded"></div>
      </div>
    );
  }

  if (sprints.length === 0) {
    return (
      <div className="flex items-center gap-2 px-3 py-2 text-sm text-muted-foreground bg-muted rounded-lg">
        <span className="material-symbols-outlined text-[16px]">sprint</span>
        No sprints
      </div>
    );
  }

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className={`flex items-center gap-2 px-3 py-2 rounded-lg border transition-all ${
          isOpen
            ? 'border-primary bg-primary/5 dark:bg-primary/10'
            : 'border-border bg-card hover:border-border/80'
        }`}
      >
        <span className={`size-2 rounded-full ${statusStyle.dot}`}></span>
        <span className="text-sm font-medium text-card-foreground">
          {displaySprint?.name || 'Select Sprint'}
        </span>
        {displaySprint && normalizeSprintStatus(displaySprint.status) === 'active' && (
          <span className="text-[10px] font-bold uppercase px-1.5 py-0.5 rounded bg-green-100 dark:bg-green-500/20 text-green-600 dark:text-green-400">
            Active
          </span>
        )}
        <span className="material-symbols-outlined text-[18px] text-muted-foreground">
          {isOpen ? 'expand_less' : 'expand_more'}
        </span>
      </button>

      {isOpen && (
        <div className="absolute top-full left-0 mt-1 w-72 bg-card border border-border rounded-lg shadow-lg z-50 overflow-hidden">
          <div className="p-2 border-b border-border">
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider px-2">
              Sprints
            </span>
          </div>
          <div className="max-h-64 overflow-y-auto">
            {/* All Sprints option */}
            <button
              onClick={() => {
                onSelectSprint(null);
                setIsOpen(false);
              }}
              className={`w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-muted transition-colors ${
                selectedSprintId === null ? 'bg-primary/5 dark:bg-primary/10' : ''
              }`}
            >
              <span className="size-2 rounded-full bg-muted-foreground/50"></span>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium text-card-foreground">All Sprints</div>
                <div className="text-xs text-muted-foreground">View all tasks</div>
              </div>
              {selectedSprintId === null && (
                <span className="material-symbols-outlined text-primary text-[18px]">check</span>
              )}
            </button>

            {/* Sprint list */}
            {sortedSprints.map((sprint) => {
              const sprintStatus = normalizeSprintStatus(sprint.status);
              const style = statusColors[sprintStatus] || statusColors.planned;
              const isSelected = sprint.id === selectedSprintId;
              const dateRange = formatSprintDates(sprint);

              return (
                <button
                  key={sprint.id}
                  onClick={() => {
                    onSelectSprint(sprint.id);
                    setIsOpen(false);
                  }}
                  className={`w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-muted transition-colors ${
                    isSelected ? 'bg-primary/5 dark:bg-primary/10' : ''
                  }`}
                >
                  <span className={`size-2 rounded-full ${style.dot}`}></span>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-card-foreground truncate">
                        {sprint.name}
                      </span>
                      {sprintStatus === 'active' && (
                        <span className={`text-[10px] font-bold uppercase px-1.5 py-0.5 rounded ${style.bg} ${style.text}`}>
                          Active
                        </span>
                      )}
                    </div>
                    {dateRange && (
                      <div className="text-xs text-muted-foreground">{dateRange}</div>
                    )}
                  </div>
                  {isSelected && (
                    <span className="material-symbols-outlined text-primary text-[18px]">check</span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
