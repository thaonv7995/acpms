import { User } from 'lucide-react';
import type { UserMessageEntry } from '@/types/timeline-log';
import { formatTimestamp } from '@/utils/formatters';

interface UserMessageCardProps {
  message: UserMessageEntry;
}

/**
 * User message card for timeline.
 * Simple card displaying user input with distinct styling.
 */
export function UserMessageCard({ message }: UserMessageCardProps) {
  return (
    <div className="relative pl-12">
      {/* Timeline dot */}
      <div
        className="absolute left-[1.875rem] top-3 w-3 h-3 rounded-full border-2 border-background bg-primary"
        aria-hidden="true"
      />

      {/* Card */}
      <div className="border border-primary/30 rounded-lg overflow-hidden bg-primary/5">
        <div className="px-4 py-3">
          {/* Header */}
          <div className="flex items-center gap-2 mb-2">
            <User className="w-4 h-4 text-primary" />
            <span className="text-sm font-medium text-primary">User</span>
            <span className="text-xs text-muted-foreground">
              {formatTimestamp(message.timestamp)}
            </span>
          </div>

          {/* Message content */}
          <div className="text-sm text-foreground whitespace-pre-wrap break-words">
            {message.content}
          </div>
        </div>
      </div>
    </div>
  );
}
