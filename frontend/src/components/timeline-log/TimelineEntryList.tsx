import { motion } from 'framer-motion';
import type { TimelineEntry } from '@/types/timeline-log';
import { TimelineEntryRenderer } from './TimelineEntryRenderer';

interface TimelineEntryListProps {
  entries: TimelineEntry[];
}

/**
 * Renders list of timeline entries with animations.
 * Delegates to TimelineEntryRenderer for individual entry rendering.
 */
export function TimelineEntryList({ entries }: TimelineEntryListProps) {
  return (
    <div className="space-y-3 px-4 py-4">
      {entries.map((entry, index) => (
        <motion.div
          key={entry.id}
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.2, delay: Math.min(index * 0.02, 0.5) }}
        >
          <TimelineEntryRenderer entry={entry} />
        </motion.div>
      ))}

      {entries.length === 0 && (
        <div className="flex items-center justify-center py-12 text-muted-foreground">
          <div className="text-center">
            <p className="text-sm">No timeline entries yet</p>
            <p className="text-xs mt-1">Entries will appear as the agent executes</p>
          </div>
        </div>
      )}
    </div>
  );
}
