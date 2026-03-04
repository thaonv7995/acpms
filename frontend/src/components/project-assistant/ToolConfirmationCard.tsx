// PA-304: ToolConfirmationCard - hiển thị tool_calls, nút Confirm/Reject
import { useState } from 'react';
import { confirmTool, type ToolCall } from '@/api/projectAssistant';

interface ToolConfirmationCardProps {
  projectId: string;
  sessionId: string;
  toolCall: ToolCall;
  onConfirmed: () => void;
}

function getToolLabel(name: string): string {
  if (name === 'create_requirement') return 'Create requirement';
  if (name === 'create_task') return 'Create task';
  return name;
}

function getToolPreview(tool: ToolCall): string {
  const title = (tool.args?.title as string) || '';
  if (tool.name === 'create_requirement') {
    return `Agent proposes to create requirement: "${title}"`;
  }
  if (tool.name === 'create_task') {
    return `Agent proposes to create task: "${title}"`;
  }
  return JSON.stringify(tool.args);
}

export function ToolConfirmationCard({
  projectId,
  sessionId,
  toolCall,
  onConfirmed,
}: ToolConfirmationCardProps) {
  const [status, setStatus] = useState<'pending' | 'confirming' | 'rejecting' | 'done'>('pending');
  const [error, setError] = useState<string | null>(null);

  const handleConfirm = async () => {
    if (status !== 'pending') return;
    setStatus('confirming');
    setError(null);
    try {
      await confirmTool(projectId, sessionId, toolCall.id, true);
      setStatus('done');
      onConfirmed();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Confirmation failed');
      setStatus('pending');
    }
  };

  const handleReject = async () => {
    if (status !== 'pending') return;
    setStatus('rejecting');
    setError(null);
    try {
      await confirmTool(projectId, sessionId, toolCall.id, false);
      setStatus('done');
      onConfirmed();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Rejection failed');
      setStatus('pending');
    }
  };

  if (status === 'done') {
    return null;
  }

  return (
    <div className="mt-2 rounded-lg border border-border bg-muted/50 p-3 text-sm">
      <p className="text-card-foreground font-medium">{getToolLabel(toolCall.name)}</p>
      <p className="mt-1 text-muted-foreground">{getToolPreview(toolCall)}</p>
      {error && <p className="mt-1 text-destructive text-xs">{error}</p>}
      <div className="mt-2 flex gap-2">
        <button
          onClick={handleConfirm}
          disabled={status !== 'pending'}
          className="px-3 py-1.5 bg-primary hover:bg-primary/90 disabled:opacity-50 text-primary-foreground rounded text-xs font-medium"
        >
          {status === 'confirming' ? 'Processing...' : 'Confirm'}
        </button>
        <button
          onClick={handleReject}
          disabled={status !== 'pending'}
          className="px-3 py-1.5 border border-border hover:bg-muted disabled:opacity-50 rounded text-xs font-medium"
        >
          {status === 'rejecting' ? 'Processing...' : 'Reject'}
        </button>
      </div>
    </div>
  );
}
