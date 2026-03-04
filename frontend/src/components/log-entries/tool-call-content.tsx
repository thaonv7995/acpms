import type { ActionType } from '@/bindings/ActionType';
import { FileReadEntry } from './tool-calls/FileReadEntry';
import { FileEditEntry } from './tool-calls/FileEditEntry';
import { CommandRunEntry } from './tool-calls/CommandRunEntry';
import { SearchEntry } from './tool-calls/SearchEntry';
import { WebFetchEntry } from './tool-calls/WebFetchEntry';
import { TaskCreateEntry } from './tool-calls/TaskCreateEntry';
import { TodoManagementEntry } from './tool-calls/TodoManagementEntry';
import { GenericToolEntry } from './tool-calls/GenericToolEntry';

/**
 * Render full details for expanded tool section
 */
export function ToolContent({
  actionType,
  content,
}: {
  actionType: ActionType;
  content: string;
}) {
  try {
    switch (actionType.action) {
      case 'file_read':
        return <FileReadEntry path={actionType.path} content={content} />;

      case 'file_edit':
        return <FileEditEntry path={actionType.path} content={content} />;

      case 'command_run':
        return (
          <CommandRunEntry
            command={actionType.command}
            output={content}
            result={actionType.result}
          />
        );

      case 'search':
        return <SearchEntry query={actionType.query} results={content} />;

      case 'web_fetch':
        return <WebFetchEntry url={actionType.url} response={content} />;

      case 'task_create':
        return <TaskCreateEntry taskData={content} />;

      case 'todo_management':
        return <TodoManagementEntry action={actionType.operation} data={content} />;

      case 'plan_presentation':
        return <GenericToolEntry actionData={content} />;

      case 'tool':
        return <GenericToolEntry actionData={content} />;

      case 'other':
        return <GenericToolEntry actionData={content} />;

      default:
        return <GenericToolEntry actionData={content} />;
    }
  } catch (error) {
    return (
      <div className="text-xs text-destructive">
        Error rendering tool details: {error instanceof Error ? error.message : 'Unknown'}
      </div>
    );
  }
}
