import { cn } from '@/lib/utils';

interface TaskCardHeaderProps {
  title: string;
  right?: React.ReactNode;
}

export function TaskCardHeader({
  title,
  right,
}: TaskCardHeaderProps) {
  return (
    <div className="flex items-center justify-between gap-2">
      {/* Left side: Title */}
      <div className="flex items-center gap-2 min-w-0 flex-1">
        <h3
          className={cn(
            'text-sm font-medium',
            'truncate',
            'text-foreground'
          )}
          title={title}
        >
          {title}
        </h3>
      </div>

      {/* Right side: Action icons with hover state */}
      {right && (
        <div className="flex items-center gap-1 opacity-100 md:opacity-0 md:group-hover:opacity-100 transition-opacity duration-200">
          {right}
        </div>
      )}
    </div>
  );
}
