import { useState, useEffect } from 'react';
import { TaskAttempt, getTaskAttempts, createTaskAttempt } from '../../api/taskAttempts';
import { AgentStatus } from './AgentStatus';
import { ExecutionLogs } from './ExecutionLogs';
import { ApiError } from '../../api/client';

interface ExecutionHistoryProps {
  taskId: string;
}

export function ExecutionHistory({ taskId }: ExecutionHistoryProps) {
  const [attempts, setAttempts] = useState<TaskAttempt[]>([]);
  const [selectedAttempt, setSelectedAttempt] = useState<TaskAttempt | null>(null);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    loadAttempts();
    // Poll for updates every 5 seconds
    const interval = setInterval(loadAttempts, 5000);
    return () => clearInterval(interval);
  }, [taskId]);

  const loadAttempts = async () => {
    try {
      const data = await getTaskAttempts(taskId);
      setAttempts(data);
      // Auto-select the latest attempt if none selected
      if (!selectedAttempt && data.length > 0) {
        setSelectedAttempt(data[0]);
      } else if (selectedAttempt) {
        // Update selected attempt with latest data
        const updated = data.find(a => a.id === selectedAttempt.id);
        if (updated) {
          setSelectedAttempt(updated);
        }
      }
      setLoading(false);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(err.message);
      } else {
        setError('Failed to load execution history');
      }
      setLoading(false);
    }
  };

  const handleStartExecution = async () => {
    setCreating(true);
    setError('');

    try {
      const newAttempt = await createTaskAttempt(taskId);
      setAttempts([newAttempt, ...attempts]);
      setSelectedAttempt(newAttempt);
    } catch (err) {
      if (err instanceof ApiError) {
        setError(err.message);
      } else {
        setError('Failed to start execution');
      }
    } finally {
      setCreating(false);
    }
  };

  const formatDate = (date: string) => {
    return new Date(date).toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  if (loading) {
    return (
      <div className="flex justify-center items-center p-8">
        <div className="text-gray-500">Loading execution history...</div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {error && (
        <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded">
          {error}
        </div>
      )}

      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-gray-900">
          Agent Execution
        </h3>
        <button
          onClick={handleStartExecution}
          disabled={creating || attempts.some(a => a.status === 'QUEUED' || a.status === 'RUNNING')}
          className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          {creating ? 'Starting...' : 'Start Execution'}
        </button>
      </div>

      {attempts.length === 0 ? (
        <div className="text-center py-12 bg-gray-50 rounded-lg border border-gray-200">
          <p className="text-gray-500 mb-4">No executions yet</p>
          <button
            onClick={handleStartExecution}
            disabled={creating}
            className="text-blue-600 hover:text-blue-700 font-medium"
          >
            {creating ? 'Starting execution...' : 'Start first execution'}
          </button>
        </div>
      ) : (
        <>
          {/* Execution history list */}
          <div className="space-y-2">
            {attempts.map((attempt) => (
              <div
                key={attempt.id}
                onClick={() => setSelectedAttempt(attempt)}
                className={`p-3 rounded-lg border cursor-pointer transition-colors ${
                  selectedAttempt?.id === attempt.id
                    ? 'border-blue-300 bg-blue-50'
                    : 'border-gray-200 bg-white hover:bg-gray-50'
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <AgentStatus
                      status={attempt.status}
                      startedAt={attempt.started_at}
                      completedAt={attempt.completed_at}
                    />
                    <span className="text-sm text-gray-600">
                      {formatDate(attempt.created_at)}
                    </span>
                  </div>
                  {selectedAttempt?.id === attempt.id && (
                    <svg className="w-5 h-5 text-blue-600" fill="currentColor" viewBox="0 0 20 20">
                      <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                    </svg>
                  )}
                </div>
                {attempt.error_message && (
                  <div className="mt-2 text-sm text-red-600">
                    Error: {attempt.error_message}
                  </div>
                )}
              </div>
            ))}
          </div>

          {/* Selected attempt logs */}
          {selectedAttempt && (
            <div className="mt-4">
              <h4 className="text-sm font-medium text-gray-700 mb-2">Execution Logs</h4>
              <ExecutionLogs attemptId={selectedAttempt.id} />
            </div>
          )}
        </>
      )}
    </div>
  );
}
