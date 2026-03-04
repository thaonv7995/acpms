/**
 * Vibe Kanban-style inline todo list.
 * Header row (icon + title + chevron) + expandable list.
 */
import { ListChecks, ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';
import { CheckCircle2, Circle, CircleDot } from 'lucide-react';
import { timelineT } from './timeline-i18n';

export interface TodoItemLike {
  content: string;
  status?: string | null;
}

interface ChatTodoListProps {
  todos: TodoItemLike[];
  expanded?: boolean;
  onToggle?: () => void;
}

function getStatusIcon(status?: string | null) {
  const s = (status || '').toLowerCase();
  if (s === 'completed')
    return <CheckCircle2 aria-hidden className="h-4 w-4 text-emerald-500" />;
  if (s === 'in_progress' || s === 'in-progress')
    return <CircleDot aria-hidden className="h-4 w-4 text-primary" />;
  if (s === 'cancelled')
    return <Circle aria-hidden className="h-4 w-4 text-muted-foreground" />;
  return <Circle aria-hidden className="h-4 w-4 text-muted-foreground" />;
}

export function ChatTodoList({ todos, expanded, onToggle }: ChatTodoListProps) {
  return (
    <div className="text-sm">
      <div
        className="flex items-center gap-2 text-muted-foreground cursor-pointer"
        onClick={onToggle}
        role="button"
      >
        <ListChecks className="shrink-0 h-4 w-4" />
        <span className="flex-1">{timelineT.updatedTodos}</span>
        <ChevronDown
          className={cn(
            'shrink-0 h-4 w-4 transition-transform',
            expanded && 'rotate-180'
          )}
        />
      </div>
      {expanded && todos.length > 0 && (
        <ul className="pt-2 ml-6 space-y-1 [&>li+li]:pt-1">
          {todos.map((todo, index) => (
            <li
              key={`${todo.content}-${index}`}
              className="flex items-start gap-2"
            >
              <span className="pt-0.5 h-4 w-4 flex items-center justify-center shrink-0">
                {getStatusIcon(todo.status)}
              </span>
              <span className="leading-5 break-words">
                {todo.status?.toLowerCase() === 'cancelled' ? (
                  <s className="text-muted-foreground">{todo.content}</s>
                ) : (
                  todo.content
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
