import type { ActionType } from '@/bindings/ActionType';

/**
 * Render one-line summary of tool action (varies by type)
 */
export function ToolSummary({ actionType }: { actionType: ActionType }) {
  switch (actionType.action) {
    case 'file_read':
      return (
        <div className="text-xs text-muted-foreground font-mono truncate">
          Read: {actionType.path}
        </div>
      );

    case 'file_edit':
      return (
        <div className="text-xs text-muted-foreground font-mono truncate">
          Edit: {actionType.path}
        </div>
      );

    case 'command_run':
      return (
        <div className="text-xs text-muted-foreground font-mono truncate">
          {actionType.command.slice(0, 60)}
          {actionType.command.length > 60 ? '...' : ''}
        </div>
      );

    case 'search':
      return (
        <div className="text-xs text-muted-foreground truncate">
          Query: {actionType.query}
        </div>
      );

    case 'web_fetch':
      return (
        <div className="text-xs text-muted-foreground truncate">
          {actionType.url}
        </div>
      );

    case 'task_create':
      return (
        <div className="text-xs text-muted-foreground truncate">
          Create: {actionType.description}
        </div>
      );

    case 'todo_management':
      return (
        <div className="text-xs text-muted-foreground">
          {actionType.operation.toUpperCase()}
        </div>
      );

    case 'plan_presentation':
      return (
        <div className="text-xs text-muted-foreground truncate">
          Plan: {actionType.plan.slice(0, 60)}
          {actionType.plan.length > 60 ? '...' : ''}
        </div>
      );

    case 'tool':
      return (
        <div className="text-xs text-muted-foreground truncate">
          {actionType.tool_name}
        </div>
      );

    case 'other':
      return (
        <div className="text-xs text-muted-foreground truncate">
          {actionType.description}
        </div>
      );

    default:
      return null;
  }
}
