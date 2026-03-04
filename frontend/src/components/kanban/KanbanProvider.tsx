import React, { useState, useCallback } from 'react';
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  DragStartEvent,
  closestCorners,
  PointerSensor,
  TouchSensor,
  KeyboardSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import { statusOrder } from '../../utils/statusLabels';
import { logger } from '@/lib/logger';

interface KanbanProviderProps {
  children: React.ReactNode;
  onTaskMove?: (taskId: string, newStatus: string) => Promise<void>;
  onDragStart?: (taskId: string) => void;
  onDragEnd?: () => void;
}

/**
 * KanbanProvider - Drag-drop context wrapper for kanban board
 *
 * Wraps kanban board with DndContext and handles drag-drop events.
 * Uses @dnd-kit for modern drag-drop with touch/keyboard support.
 */
export function KanbanProvider({
  children,
  onTaskMove,
  onDragStart,
  onDragEnd,
}: KanbanProviderProps) {
  const [activeId, setActiveId] = useState<string | null>(null);
  const columnCount = React.Children.count(children);
  const minColumnWidth = 240;

  // Configure sensors for different input methods
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8, // Drag distance before activating
      },
    }),
    useSensor(TouchSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor)
  );

  // Handle drag start
  const handleDragStart = useCallback(
    (event: DragStartEvent) => {
      const { active } = event;
      setActiveId(active.id as string);
      onDragStart?.(active.id as string);
    },
    [onDragStart]
  );

  // Handle drag end - update task status via API
  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      setActiveId(null);

      if (!over) {
        onDragEnd?.();
        return;
      }

      // Extract column status from drop zone ID (format: "column-{status}")
      const overColumnId = over.id as string;
      if (!overColumnId.startsWith('column-')) {
        onDragEnd?.();
        return;
      }

      // Parse status from column ID (e.g., "column-in_progress" -> "in_progress")
      const newStatus = overColumnId.replace('column-', '');

      // Validate status
      if (!isValidStatus(newStatus)) {
        logger.warn(`Invalid status: ${newStatus}`);
        onDragEnd?.();
        return;
      }

      // Call move handler if provided
      try {
        await onTaskMove?.(active.id as string, newStatus);
      } catch (error) {
        logger.error('Failed to move task:', error);
        // Error handling will be managed by parent component
      }

      onDragEnd?.();
    },
    [onTaskMove, onDragEnd]
  );

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCorners}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
    >
      <div
        className="grid w-full min-h-full divide-x divide-border border-x border-border items-stretch"
        style={{
          gridTemplateColumns: `repeat(${Math.max(columnCount, 1)}, minmax(${minColumnWidth}px, 1fr))`,
          minWidth: `${Math.max(columnCount, 1) * minColumnWidth}px`,
        }}
      >
        {children}
      </div>
      <DragOverlay>
        {activeId && (
          <div className="bg-blue-100 dark:bg-blue-900 border-2 border-blue-500 rounded-lg p-3 shadow-2xl">
            <p className="text-sm font-medium text-blue-900 dark:text-blue-100">
              Moving task...
            </p>
          </div>
        )}
      </DragOverlay>
    </DndContext>
  );
}

/**
 * Validate if a string is a valid task status
 */
function isValidStatus(value: unknown): value is string {
  return typeof value === 'string' && statusOrder.includes(value as any);
}

export type { KanbanProviderProps };
