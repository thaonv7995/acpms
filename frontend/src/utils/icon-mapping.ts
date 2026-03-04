import {
  Eye,
  Edit,
  FileText,
  Terminal,
  Search,
  Globe,
  CheckSquare,
  ListTodo,
  AlertCircle,
  Wrench,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

export type ActionTypeKey =
  | 'file_read'
  | 'file_edit'
  | 'file_write'
  | 'command_run'
  | 'search'
  | 'web_fetch'
  | 'task_create'
  | 'todo_management'
  | 'tool'
  | 'plan_presentation'
  | 'other'
  | 'unknown';

/**
 * Map action types to appropriate lucide-react icons
 */
const actionIconMap: Record<ActionTypeKey, LucideIcon> = {
  file_read: Eye,
  file_edit: Edit,
  file_write: FileText,
  command_run: Terminal,
  search: Search,
  web_fetch: Globe,
  task_create: CheckSquare,
  todo_management: ListTodo,
  tool: Wrench,
  plan_presentation: FileText,
  other: AlertCircle,
  unknown: AlertCircle,
};

/**
 * Get icon component for action type
 */
export function getActionIcon(action: string): LucideIcon {
  const normalizedAction = action.toLowerCase() as ActionTypeKey;
  return actionIconMap[normalizedAction] || actionIconMap.unknown;
}

/**
 * Get label for action type
 */
export function getActionLabel(action: string): string {
  const labels: Record<ActionTypeKey, string> = {
    file_read: 'Read File',
    file_edit: 'Edit File',
    file_write: 'Write File',
    command_run: 'Run Command',
    search: 'Search',
    web_fetch: 'Fetch URL',
    task_create: 'Create Task',
    todo_management: 'Todo Action',
    tool: 'Tool',
    plan_presentation: 'Plan Presentation',
    other: 'Action',
    unknown: 'Tool Action',
  };

  const normalizedAction = action.toLowerCase() as ActionTypeKey;
  return labels[normalizedAction] || 'Tool Action';
}
