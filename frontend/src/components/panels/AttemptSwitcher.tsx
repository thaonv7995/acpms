import { ChevronDown, History } from 'lucide-react';
import { cn } from '@/lib/utils';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import type { TaskAttempt } from '@/types/task-attempt';
import { useElapsedRealtime } from '@/hooks/useElapsedRealtime';

interface AttemptSwitcherProps {
  currentAttempt: TaskAttempt;
  allAttempts: TaskAttempt[];
  onSelectAttempt: (attemptId: string) => void;
  onViewHistory?: () => void;
}

export function AttemptSwitcher({
  currentAttempt,
  allAttempts,
  onSelectAttempt,
  onViewHistory,
}: AttemptSwitcherProps) {
  const currentIndex = allAttempts.findIndex((a) => a.id === currentAttempt.id);
  const currentAttemptNumber = allAttempts.length - currentIndex;
  const isRunning = currentAttempt.status === 'running';
  const elapsed = useElapsedRealtime(
    currentAttempt.started_at ?? null,
    isRunning
  );

  // Sort attempts newest first for dropdown display
  const sortedAttempts = [...allAttempts].sort(
    (a, b) =>
      new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
  );

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="sm" className="gap-2">
          <span className="font-semibold">Attempt #{currentAttemptNumber}</span>
          {isRunning && elapsed && (
            <span className="text-muted-foreground font-normal" title="Elapsed">
              ({elapsed})
            </span>
          )}
          <ChevronDown className="h-3 w-3 opacity-50" />
        </Button>
      </DropdownMenuTrigger>

      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuLabel className="text-xs font-semibold uppercase tracking-wide">
          Switch Attempt
        </DropdownMenuLabel>
        <DropdownMenuSeparator />

        {sortedAttempts.length === 0 ? (
          <div className="px-2 py-1.5 text-xs text-muted-foreground text-center">
            No attempts yet
          </div>
        ) : (
          sortedAttempts.map((attempt, index) => {
            const num = sortedAttempts.length - index;
            const isCurrent = attempt.id === currentAttempt.id;

            return (
              <DropdownMenuItem
                key={attempt.id}
                onClick={() => onSelectAttempt(attempt.id)}
                className={cn(
                  isCurrent && 'bg-accent'
                )}
              >
                <div className="flex items-center justify-between w-full gap-2">
                  <span className="font-medium">Attempt #{num}</span>
                  <span className="text-xs text-muted-foreground">
                    {attempt.status}
                  </span>
                </div>
              </DropdownMenuItem>
            );
          })
        )}

        {onViewHistory && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={onViewHistory} className="gap-2">
              <History className="h-4 w-4" />
              <span>View Full History</span>
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
