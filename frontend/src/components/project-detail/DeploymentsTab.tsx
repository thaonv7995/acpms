import { useCallback, useEffect, useMemo, useState } from 'react';
import { ScrollText } from 'lucide-react';
import {
  cancelDeploymentRun,
  listDeploymentEnvironments,
  listDeploymentRunLogs,
  listDeploymentRunTimeline,
  listDeploymentRuns,
  retryDeploymentRun,
  startDeploymentRun,
  type DeploymentEnvironment,
  type DeploymentRun,
  type DeploymentRunStatus,
  type DeploymentSourceType,
  type DeploymentTimelineEvent,
} from '../../api/deploymentEnvironments';
import { DeploymentEnvironmentsSettings } from './DeploymentEnvironmentsSettings';
import { AgentLogsSection } from '../task-detail-page/AgentLogsSection';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '../ui/dialog';

interface DeploymentsTabProps {
  projectId: string;
}

function statusBadgeClass(status: DeploymentRunStatus): string {
  switch (status) {
    case 'success':
      return 'bg-green-100 text-green-700 dark:bg-green-500/20 dark:text-green-300';
    case 'failed':
    case 'cancelled':
      return 'bg-red-100 text-red-700 dark:bg-red-500/20 dark:text-red-300';
    case 'running':
    case 'rolling_back':
      return 'bg-blue-100 text-blue-700 dark:bg-blue-500/20 dark:text-blue-300';
    case 'queued':
      return 'bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300';
    default:
      return 'bg-muted text-muted-foreground';
  }
}

function toLocalDatetime(value?: string | null): string {
  if (!value) return '-';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export function DeploymentsTab({ projectId }: DeploymentsTabProps) {
  const [environments, setEnvironments] = useState<DeploymentEnvironment[]>([]);
  const [runs, setRuns] = useState<DeploymentRun[]>([]);
  const [logs, setLogs] = useState<DeploymentTimelineEvent[]>([]);
  const [timeline, setTimeline] = useState<DeploymentTimelineEvent[]>([]);

  const [loading, setLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const [selectedEnvironmentId, setSelectedEnvironmentId] = useState<string>('');
  const [logsModalRunId, setLogsModalRunId] = useState<string | null>(null);

  const runForLogsModal = useMemo(
    () => (logsModalRunId ? runs.find((r) => r.id === logsModalRunId) ?? null : null),
    [logsModalRunId, runs]
  );

  const [sourceType, setSourceType] = useState<DeploymentSourceType>('branch');
  const [sourceRef, setSourceRef] = useState('main');

  const syncEnvironments = useCallback((data: DeploymentEnvironment[]) => {
    setEnvironments(data);
    setSelectedEnvironmentId((current) => {
      if (data.length === 0) return '';
      if (current && data.some((env) => env.id === current)) return current;
      const fallback = data.find((env) => env.is_default) ?? data[0];
      return fallback.id;
    });
  }, []);

  const selectedEnvironment = useMemo(
    () => environments.find((env) => env.id === selectedEnvironmentId) ?? null,
    [environments, selectedEnvironmentId]
  );

  const loadEnvironments = useCallback(async () => {
    const data = await listDeploymentEnvironments(projectId);
    syncEnvironments(data);
  }, [projectId, syncEnvironments]);

  const loadRuns = useCallback(async () => {
    if (!selectedEnvironmentId) return;
    const data = await listDeploymentRuns(projectId, {
      environment_id: selectedEnvironmentId,
      limit: 100,
    });
    setRuns(data);
  }, [projectId, selectedEnvironmentId]);

  const loadLogsForRun = useCallback(async (runId: string) => {
    const [timelineData, logsData] = await Promise.all([
      listDeploymentRunTimeline(runId),
      listDeploymentRunLogs(runId),
    ]);
    setTimeline(timelineData);
    setLogs(logsData);
  }, []);

  const loadInitialData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      await loadEnvironments();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load deployment data');
    } finally {
      setLoading(false);
    }
  }, [loadEnvironments]);

  useEffect(() => {
    void loadInitialData();
  }, [loadInitialData]);

  useEffect(() => {
    if (!selectedEnvironmentId) {
      setRuns([]);
      setLogs([]);
      setTimeline([]);
      setLogsModalRunId(null);
    }
  }, [selectedEnvironmentId]);

  useEffect(() => {
    if (!selectedEnvironmentId) return;
    void loadRuns();
  }, [loadRuns, selectedEnvironmentId]);

  const currentRun = useMemo(() => {
    const visible = runs.filter((r) => r.status !== 'cancelled' && r.status !== 'rolled_back');
    return visible.sort(
      (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
    )[0] ?? null;
  }, [runs]);

  const isRunning = currentRun && ['queued', 'running', 'rolling_back'].includes(currentRun.status);

  const needsLogsPoll =
    !!logsModalRunId &&
    runForLogsModal &&
    !runForLogsModal.attempt_id &&
    ['queued', 'running', 'rolling_back'].includes(runForLogsModal.status);

  const pollInterval = isRunning || needsLogsPoll ? 2000 : 4000;

  useEffect(() => {
    if (!selectedEnvironmentId) return;
    const refresh = async () => {
      await loadRuns();
      if (needsLogsPoll && logsModalRunId) {
        await loadLogsForRun(logsModalRunId);
      }
    };
    const id = window.setInterval(() => void refresh(), pollInterval);
    return () => window.clearInterval(id);
  }, [selectedEnvironmentId, loadRuns, loadLogsForRun, pollInterval, needsLogsPoll, logsModalRunId]);

  useEffect(() => {
    if (!logsModalRunId || runForLogsModal?.attempt_id) return;
    void loadLogsForRun(logsModalRunId);
  }, [logsModalRunId, runForLogsModal?.attempt_id, loadLogsForRun]);

  const runAction = async (action: () => Promise<void>, successMessage: string, busyKey: string) => {
    setBusyAction(busyKey);
    setError(null);
    setMessage(null);
    try {
      await action();
      await loadRuns();
      setMessage(successMessage);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Action failed');
    } finally {
      setBusyAction(null);
    }
  };

  const startDeploy = async () => {
    if (!selectedEnvironmentId) return;
    await runAction(
      async () => {
        await startDeploymentRun(projectId, selectedEnvironmentId, {
          source_type: sourceType,
          source_ref: sourceRef.trim() || undefined,
        });
      },
      'Deploy has been started.',
      'start'
    );
  };

  const cancelRunById = async (runId: string) => {
    await runAction(
      async () => {
        await cancelDeploymentRun(runId);
      },
      'Deploy has been cancelled.',
      'cancel'
    );
    if (logsModalRunId === runId) setLogsModalRunId(null);
  };

  const retryRunById = async (runId: string) => {
    await runAction(
      async () => {
        await retryDeploymentRun(runId);
      },
      'Retry has been started.',
      'retry'
    );
  };

  if (loading) {
    return <p className="text-sm text-muted-foreground">Loading deployment data...</p>;
  }

  return (
    <div className="space-y-4">
      {error && (
        <div className="px-3 py-2 text-sm rounded-lg border border-red-200 bg-red-50 text-red-700 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-300">
          {error}
        </div>
      )}
      {message && (
        <div className="px-3 py-2 text-sm rounded-lg border border-green-200 bg-green-50 text-green-700 dark:border-green-500/30 dark:bg-green-500/10 dark:text-green-300">
          {message}
        </div>
      )}

      <DeploymentEnvironmentsSettings
        projectId={projectId}
        environments={environments}
        loading={loading}
        onEnvironmentsChanged={syncEnvironments}
      />

      <div className="bg-card border border-border rounded-xl p-4">
        <h3 className="text-base font-semibold text-card-foreground mb-3">Deploy</h3>
        <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
          <label className="text-sm text-card-foreground">
            Environment
            <select
              value={selectedEnvironmentId}
              onChange={(e) => setSelectedEnvironmentId(e.target.value)}
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
            >
              {environments.length === 0 && <option value="">No environment</option>}
              {environments.map((env) => (
                <option key={env.id} value={env.id}>
                  {env.name}
                </option>
              ))}
            </select>
          </label>

          <label className="text-sm text-card-foreground">
            Source Type
            <select
              value={sourceType}
              onChange={(e) => setSourceType(e.target.value as DeploymentSourceType)}
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
            >
              <option value="branch">branch</option>
              <option value="commit">commit</option>
              <option value="artifact">artifact</option>
              <option value="release">release</option>
            </select>
          </label>

          <label className="text-sm text-card-foreground md:col-span-2">
            Source Ref
            <input
              type="text"
              value={sourceRef}
              onChange={(e) => setSourceRef(e.target.value)}
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
              placeholder="main"
            />
          </label>
        </div>

        <div className="mt-3 flex items-center gap-3">
          <button
            onClick={() => void startDeploy()}
            disabled={!selectedEnvironmentId || busyAction !== null}
            className="px-4 py-2 bg-primary text-primary-foreground text-sm rounded-lg disabled:opacity-60"
          >
            {busyAction === 'start' ? 'Starting...' : 'Start Deploy'}
          </button>
        </div>
      </div>

      {currentRun && (
        <div className="bg-card border border-border rounded-xl p-4">
          <div className="flex items-start justify-between gap-3">
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 flex-wrap">
                <span
                  className={`text-xs px-2 py-0.5 rounded ${statusBadgeClass(currentRun.status)}`}
                >
                  {currentRun.status}
                </span>
                <span className="text-sm text-muted-foreground">
                  {currentRun.source_type}:{currentRun.source_ref || '-'}
                </span>
                <span className="text-xs text-muted-foreground">
                  {toLocalDatetime(currentRun.started_at || currentRun.created_at)}
                </span>
              </div>

              {selectedEnvironment && currentRun.status === 'success' && (() => {
                const tc = selectedEnvironment.target_config as Record<string, unknown> | undefined;
                const host = (tc?.host ?? tc?.hostname) as string | undefined;
                const primaryDomain = selectedEnvironment.domain_config &&
                  typeof selectedEnvironment.domain_config === 'object'
                  ? (selectedEnvironment.domain_config as Record<string, unknown>).primary_domain as string | undefined
                  : undefined;
                const healthcheckUrl = selectedEnvironment.healthcheck_url ?? undefined;
                const appUrl = primaryDomain
                  ? (primaryDomain.startsWith('http') ? primaryDomain : `https://${primaryDomain}`)
                  : healthcheckUrl;
                return (
                  <div className="mt-3 p-3 rounded-lg border border-emerald-200 bg-emerald-50 dark:border-emerald-500/30 dark:bg-emerald-500/10 space-y-1 text-sm">
                    {appUrl && (
                      <p>
                        <span className="text-muted-foreground">URL: </span>
                        <a
                          href={appUrl}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="font-mono text-primary hover:underline break-all"
                        >
                          {appUrl}
                        </a>
                      </p>
                    )}
                    {(host || selectedEnvironment.target_type === 'local') && (
                      <p>
                        <span className="text-muted-foreground">Server: </span>
                        <span className="font-mono">{host || 'local'}</span>
                      </p>
                    )}
                    <p>
                      <span className="text-muted-foreground">Path: </span>
                      <span className="font-mono break-all">{selectedEnvironment.deploy_path}</span>
                    </p>
                  </div>
                );
              })()}

              {currentRun.error_message && (
                <p className="mt-2 text-sm text-red-600 dark:text-red-400">{currentRun.error_message}</p>
              )}

              <div className="mt-3 flex flex-wrap gap-2">
                {['queued', 'running', 'rolling_back'].includes(currentRun.status) && (
                  <button
                    onClick={() => void cancelRunById(currentRun.id)}
                    disabled={busyAction !== null}
                    className="px-3 py-1.5 text-xs font-medium border border-red-300 text-red-600 rounded hover:bg-red-50 dark:border-red-500/50 dark:text-red-400 dark:hover:bg-red-500/10 disabled:opacity-60"
                  >
                    {busyAction === 'cancel' ? 'Cancelling...' : 'Cancel'}
                  </button>
                )}
                {currentRun.status === 'failed' && (
                  <button
                    onClick={() => void retryRunById(currentRun.id)}
                    disabled={busyAction !== null}
                    className="px-3 py-1.5 text-xs border border-border rounded hover:bg-muted/50 disabled:opacity-60"
                  >
                    {busyAction === 'retry' ? 'Retrying...' : 'Retry'}
                  </button>
                )}
                {(currentRun.status === 'success' || currentRun.status === 'failed') && (
                  <button
                    onClick={() => void cancelRunById(currentRun.id)}
                    disabled={busyAction !== null}
                    className="px-3 py-1.5 text-xs text-muted-foreground hover:text-card-foreground border border-border rounded hover:bg-muted/50 disabled:opacity-60"
                    title="Hide this deployment"
                  >
                    {busyAction === 'cancel' ? 'Hiding...' : 'Hide'}
                  </button>
                )}
              </div>
            </div>

            <button
              onClick={() => setLogsModalRunId(currentRun.id)}
              className="p-2 rounded-lg border border-border hover:bg-muted/50 text-muted-foreground hover:text-card-foreground"
              title="View agent log"
            >
              <ScrollText className="w-5 h-5" />
            </button>
          </div>
        </div>
      )}

      <Dialog open={!!logsModalRunId} onOpenChange={(open) => !open && setLogsModalRunId(null)}>
        <DialogContent className="max-w-2xl max-h-[80vh] overflow-hidden flex flex-col">
          <DialogHeader>
            <DialogTitle>Log deploy</DialogTitle>
          </DialogHeader>
          <div className="flex-1 overflow-y-auto min-h-0">
            {runForLogsModal?.attempt_id ? (
              <AgentLogsSection
                attemptId={runForLogsModal.attempt_id}
                status={runForLogsModal.status}
              />
            ) : (
              <div className="space-y-4">
                <div>
                  <p className="text-sm font-medium text-card-foreground mb-2">Timeline</p>
                  <div className="max-h-[200px] overflow-y-auto border border-border rounded-lg p-2 space-y-2">
                    {timeline.length === 0 ? (
                      <p className="text-xs text-muted-foreground">No timeline yet.</p>
                    ) : (
                      timeline.map((event) => (
                        <div key={event.id} className="text-xs border-b border-border/50 pb-1 last:border-0">
                          <p className="font-medium text-card-foreground">
                            {event.step} • {event.event_type}
                          </p>
                          <p className="text-muted-foreground">{event.message}</p>
                          <p className="text-muted-foreground">{toLocalDatetime(event.created_at)}</p>
                        </div>
                      ))
                    )}
                  </div>
                </div>
                <div>
                  <p className="text-sm font-medium text-card-foreground mb-2">Logs</p>
                  <div className="max-h-[240px] overflow-y-auto border border-border rounded-lg p-2 space-y-2">
                    {logs.length === 0 ? (
                      <p className="text-xs text-muted-foreground">No logs yet.</p>
                    ) : (
                      logs.map((event) => (
                        <div key={event.id} className="text-xs border-b border-border/50 pb-1 last:border-0">
                          <p className="font-medium text-card-foreground">{event.event_type}</p>
                          <p className="text-muted-foreground">{event.message}</p>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
