import { useCallback, useMemo, useState } from 'react';
import {
  createDeploymentEnvironment,
  deleteDeploymentEnvironment,
  fetchSshKnownHosts,
  listDeploymentEnvironments,
  testDeploymentEnvironmentConnection,
  testDeploymentEnvironmentDomain,
  type CreateDeploymentEnvironmentRequest,
  type DeploymentConnectionTestResponse,
  type DeploymentEnvironment,
  type DeploymentTargetType,
  updateDeploymentEnvironment,
} from '../../api/deploymentEnvironments';

interface DeploymentEnvironmentsSettingsProps {
  projectId: string;
  environments: DeploymentEnvironment[];
  loading?: boolean;
  onEnvironmentsChanged?: (environments: DeploymentEnvironment[]) => void;
}

interface EnvironmentFormState {
  id?: string;
  name: string;
  targetType: DeploymentTargetType;
  deployPath: string;
  healthcheckUrl: string;
  primaryDomain: string;
  isEnabled: boolean;
  isDefault: boolean;
  sshHost: string;
  sshPort: string;
  sshUsername: string;
  sshPassword: string;
  sshPrivateKey: string;
  sshKnownHosts: string;
}

const emptyFormState: EnvironmentFormState = {
  name: '',
  targetType: 'local',
  deployPath: '',
  healthcheckUrl: '',
  primaryDomain: '',
  isEnabled: true,
  isDefault: false,
  sshHost: '',
  sshPort: '22',
  sshUsername: '',
  sshPassword: '',
  sshPrivateKey: '',
  sshKnownHosts: '',
};

function toFormState(env: DeploymentEnvironment): EnvironmentFormState {
  const targetConfig = env.target_config || {};
  const domainConfig = env.domain_config || {};

  return {
    id: env.id,
    name: env.name,
    targetType: env.target_type,
    deployPath: env.deploy_path,
    healthcheckUrl: env.healthcheck_url ?? '',
    primaryDomain: typeof domainConfig.primary_domain === 'string' ? domainConfig.primary_domain : '',
    isEnabled: env.is_enabled,
    isDefault: env.is_default,
    sshHost: typeof targetConfig.host === 'string' ? targetConfig.host : '',
    sshPort: String(typeof targetConfig.port === 'number' ? targetConfig.port : 22),
    sshUsername:
      typeof targetConfig.username === 'string'
        ? targetConfig.username
        : typeof targetConfig.user === 'string'
          ? targetConfig.user
          : '',
    sshPassword: '',
    sshPrivateKey: '',
    sshKnownHosts: '',
  };
}

export function DeploymentEnvironmentsSettings({
  projectId,
  environments: environmentsProp,
  loading: loadingProp = false,
  onEnvironmentsChanged,
}: DeploymentEnvironmentsSettingsProps) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [formState, setFormState] = useState<EnvironmentFormState>(emptyFormState);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [actionMessageSuccess, setActionMessageSuccess] = useState<boolean | null>(null);
  const [testResults, setTestResults] = useState<Record<string, DeploymentConnectionTestResponse>>({});
  const [workingByEnvId, setWorkingByEnvId] = useState<Record<string, boolean>>({});
  const [fetchingKnownHosts, setFetchingKnownHosts] = useState(false);

  const isEditing = useMemo(() => Boolean(formState.id), [formState.id]);

  const refreshEnvironments = useCallback(async () => {
    setError(null);
    try {
      const data = await listDeploymentEnvironments(projectId);
      onEnvironmentsChanged?.(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load deployment environments');
    }
  }, [onEnvironmentsChanged, projectId]);

  const resetForm = () => {
    setFormState(emptyFormState);
    setShowForm(false);
  };

  const upsertEnvironment = async () => {
    if (!formState.name.trim() || !formState.deployPath.trim()) {
      setError('Name and deploy path are required.');
      return;
    }

    if (formState.targetType === 'ssh_remote') {
      if (!formState.sshHost.trim() || !formState.sshUsername.trim()) {
        setError('SSH Host and Username are required for ssh_remote.');
        return;
      }
      if (!formState.id) {
        const hasPassword = Boolean(formState.sshPassword.trim());
        const hasKey = Boolean(formState.sshPrivateKey.trim());
        if (!hasPassword && !hasKey) {
          setError('SSH Password or Private Key is required for authentication.');
          return;
        }
      }
      if (!formState.id && !formState.sshKnownHosts.trim()) {
        setError('SSH Known Hosts is required (host verification). Run: ssh-keyscan -H <host>');
        return;
      }
    }

    const targetConfig =
      formState.targetType === 'ssh_remote'
        ? {
            host: formState.sshHost.trim(),
            port: Number(formState.sshPort) || 22,
            username: formState.sshUsername.trim(),
          }
        : {};

    const domainConfig = formState.primaryDomain.trim()
      ? { primary_domain: formState.primaryDomain.trim() }
      : {};

    const secrets: { secret_type: 'ssh_password' | 'ssh_private_key' | 'known_hosts'; value: string }[] = [];
    if (formState.targetType === 'ssh_remote') {
      if (formState.sshKnownHosts.trim()) {
        secrets.push({ secret_type: 'known_hosts', value: formState.sshKnownHosts.trim() });
      }
      if (formState.sshPassword.trim()) {
        secrets.push({ secret_type: 'ssh_password', value: formState.sshPassword });
      }
      if (formState.sshPrivateKey.trim()) {
        secrets.push({ secret_type: 'ssh_private_key', value: formState.sshPrivateKey });
      }
    }

    const payload: CreateDeploymentEnvironmentRequest = {
      name: formState.name.trim(),
      target_type: formState.targetType,
      deploy_path: formState.deployPath.trim(),
      healthcheck_url: formState.healthcheckUrl.trim() || undefined,
      domain_config: domainConfig,
      target_config: targetConfig,
      is_enabled: formState.isEnabled,
      is_default: formState.isDefault,
      ...(secrets.length > 0 && { secrets }),
    };

    setSaving(true);
    setError(null);
    setActionMessage(null);

    try {
      if (formState.id) {
        await updateDeploymentEnvironment(projectId, formState.id, payload);
        setActionMessage(`Environment ${formState.name} updated.`);
      } else {
        await createDeploymentEnvironment(projectId, payload);
        setActionMessage(`Environment ${formState.name} created.`);
      }
      setActionMessageSuccess(true);

      await refreshEnvironments();
      resetForm();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save deployment environment');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (env: DeploymentEnvironment) => {
    if (!window.confirm(`Delete environment \"${env.name}\"?`)) {
      return;
    }

    setWorkingByEnvId((prev) => ({ ...prev, [env.id]: true }));
    setError(null);
    setActionMessage(null);

    try {
      await deleteDeploymentEnvironment(projectId, env.id);
      await refreshEnvironments();
      setActionMessage(`Environment ${env.name} deleted.`);
      setActionMessageSuccess(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete environment');
    } finally {
      setWorkingByEnvId((prev) => ({ ...prev, [env.id]: false }));
    }
  };

  const runTest = async (env: DeploymentEnvironment, type: 'connection' | 'domain') => {
    setWorkingByEnvId((prev) => ({ ...prev, [env.id]: true }));
    setActionMessage(null);
    setError(null);

    try {
      const result =
        type === 'connection'
          ? await testDeploymentEnvironmentConnection(projectId, env.id)
          : await testDeploymentEnvironmentDomain(projectId, env.id);
      setTestResults((prev) => ({ ...prev, [env.id]: result }));
      setActionMessageSuccess(result.success);
      setActionMessage(
        type === 'connection'
          ? `Connection test ${result.success ? 'passed' : 'failed'} for ${env.name}.`
          : `Domain test ${result.success ? 'passed' : 'failed'} for ${env.name}.`
      );
    } catch (err) {
      setActionMessageSuccess(false);
      setError(err instanceof Error ? err.message : 'Failed to run environment test');
    } finally {
      setWorkingByEnvId((prev) => ({ ...prev, [env.id]: false }));
    }
  };

  return (
    <div className="bg-card border border-border rounded-xl p-6 space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-lg font-bold text-card-foreground flex items-center gap-2">
            <span className="material-symbols-outlined text-primary">cloud_upload</span>
            Deployment Environments
          </h3>
          <p className="text-sm text-muted-foreground mt-1">
            Configure custom environments, local or SSH targets, and test connectivity/domain before deploy.
          </p>
        </div>
        <button
          onClick={() => {
            setError(null);
            setActionMessage(null);
            if (showForm) {
              resetForm();
            } else {
              setShowForm(true);
            }
            setActionMessageSuccess(null);
          }}
          className="px-3 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-medium rounded-lg transition-colors"
        >
          {showForm ? 'Close' : 'Add Environment'}
        </button>
      </div>

      {error && (
        <div className="px-3 py-2 text-sm rounded-lg border border-red-200 bg-red-50 text-red-700 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-300">
          {error}
        </div>
      )}
      {actionMessage && (
        <div
          className={`px-3 py-2 text-sm rounded-lg border ${
            actionMessageSuccess === true
              ? 'border-green-200 bg-green-50 text-green-700 dark:border-green-500/30 dark:bg-green-500/10 dark:text-green-300'
              : actionMessageSuccess === false
                ? 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300'
                : 'border-green-200 bg-green-50 text-green-700 dark:border-green-500/30 dark:bg-green-500/10 dark:text-green-300'
          }`}
        >
          {actionMessage}
        </div>
      )}

      {showForm && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 p-4 border border-border rounded-lg bg-muted/20">
          <label className="text-sm text-card-foreground">
            Name
            <input
              type="text"
              value={formState.name}
              onChange={(e) => setFormState((prev) => ({ ...prev, name: e.target.value }))}
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
              placeholder="staging-eu"
            />
          </label>
          <label className="text-sm text-card-foreground">
            Target Type
            <select
              value={formState.targetType}
              onChange={(e) =>
                setFormState((prev) => ({ ...prev, targetType: e.target.value as DeploymentTargetType }))
              }
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
            >
              <option value="local">local</option>
              <option value="ssh_remote">ssh_remote</option>
            </select>
          </label>
          <label className="text-sm text-card-foreground md:col-span-2">
            Deploy Path
            <input
              type="text"
              value={formState.deployPath}
              onChange={(e) => setFormState((prev) => ({ ...prev, deployPath: e.target.value }))}
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
              placeholder="/var/www/project"
            />
          </label>
          <label className="text-sm text-card-foreground md:col-span-2">
            Healthcheck URL (optional)
            <input
              type="text"
              value={formState.healthcheckUrl}
              onChange={(e) => setFormState((prev) => ({ ...prev, healthcheckUrl: e.target.value }))}
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
              placeholder="https://staging.example.com/health"
            />
          </label>
          <label className="text-sm text-card-foreground md:col-span-2">
            Primary Domain (optional)
            <input
              type="text"
              value={formState.primaryDomain}
              onChange={(e) => setFormState((prev) => ({ ...prev, primaryDomain: e.target.value }))}
              className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
              placeholder="staging.example.com"
            />
          </label>

          {formState.targetType === 'ssh_remote' && (
            <>
              <label className="text-sm text-card-foreground">
                SSH Host
                <input
                  type="text"
                  value={formState.sshHost}
                  onChange={(e) => setFormState((prev) => ({ ...prev, sshHost: e.target.value }))}
                  className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
                  placeholder="1.2.3.4"
                />
              </label>
              <label className="text-sm text-card-foreground">
                SSH Username
                <input
                  type="text"
                  value={formState.sshUsername}
                  onChange={(e) => setFormState((prev) => ({ ...prev, sshUsername: e.target.value }))}
                  className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
                  placeholder="ubuntu"
                />
              </label>
              <label className="text-sm text-card-foreground">
                SSH Port
                <input
                  type="number"
                  min={1}
                  max={65535}
                  value={formState.sshPort}
                  onChange={(e) => setFormState((prev) => ({ ...prev, sshPort: e.target.value }))}
                  className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
                />
              </label>
              <div className="text-sm text-card-foreground md:col-span-2">
                <div className="flex items-center justify-between gap-2 mb-1 flex-wrap">
                  <span>SSH Known Hosts (required)</span>
                  <div className="flex items-center gap-1.5 flex-wrap">
                    <input
                      type="text"
                      value={formState.sshHost}
                      onChange={(e) => setFormState((prev) => ({ ...prev, sshHost: e.target.value }))}
                      placeholder="Host (VD: 192.168.1.5)"
                      className="w-36 px-2 py-1.5 bg-card border border-border rounded text-xs"
                    />
                    <button
                      type="button"
                      onClick={async () => {
                        const host = formState.sshHost.trim();
                        if (!host) {
                          setError('Enter host (IP or domain) before fetching known hosts.');
                          return;
                        }
                        setFetchingKnownHosts(true);
                        setError(null);
                        try {
                          const result = await fetchSshKnownHosts(projectId, host);
                          setFormState((prev) => ({ ...prev, sshKnownHosts: result.known_hosts }));
                          setActionMessage(`Fetched known hosts for ${host}`);
                          setActionMessageSuccess(true);
                        } catch (err) {
                          setError(err instanceof Error ? err.message : 'Failed to fetch known hosts');
                        } finally {
                          setFetchingKnownHosts(false);
                        }
                      }}
                      disabled={fetchingKnownHosts}
                      className="px-2.5 py-1.5 bg-primary hover:bg-primary/90 text-primary-foreground border border-primary rounded text-xs font-medium flex items-center gap-1.5 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <span className="material-symbols-outlined text-[14px]">
                        {fetchingKnownHosts ? 'hourglass_empty' : 'download'}
                      </span>
                      {fetchingKnownHosts ? 'Fetching...' : 'Fetch from server'}
                    </button>
                    <input
                      type="file"
                      id="ssh-known-hosts-file"
                      className="hidden"
                      onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (!file) return;
                        const reader = new FileReader();
                        reader.onload = () => {
                          const text = typeof reader.result === 'string' ? reader.result : '';
                          setFormState((prev) => ({ ...prev, sshKnownHosts: text }));
                        };
                        reader.readAsText(file);
                        e.target.value = '';
                      }}
                    />
                    <label
                      htmlFor="ssh-known-hosts-file"
                      className="px-2.5 py-1.5 bg-muted hover:bg-muted/80 border border-border rounded text-xs font-medium text-card-foreground cursor-pointer flex items-center gap-1.5"
                    >
                      <span className="material-symbols-outlined text-[14px]">upload_file</span>
                      Upload file
                    </label>
                  </div>
                </div>
                <textarea
                  value={formState.sshKnownHosts}
                  onChange={(e) => setFormState((prev) => ({ ...prev, sshKnownHosts: e.target.value }))}
                  className="w-full px-3 py-2 bg-card border border-border rounded-lg font-mono text-xs min-h-[60px]"
                  placeholder={formState.id ? 'Leave empty to keep existing' : 'Paste known_hosts or run: ssh-keyscan -H <host>'}
                  rows={2}
                  spellCheck={false}
                />
                <p className="text-xs text-muted-foreground mt-0.5">Host verification — fetch via: ssh-keyscan -H &lt;host&gt;</p>
              </div>
              <label className="text-sm text-card-foreground md:col-span-2">
                SSH Password (optional)
                <input
                  type="password"
                  value={formState.sshPassword}
                  onChange={(e) => setFormState((prev) => ({ ...prev, sshPassword: e.target.value }))}
                  className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg"
                  placeholder={formState.id ? 'Leave empty to keep existing' : 'Password for SSH auth'}
                  autoComplete="new-password"
                />
                <p className="text-xs text-muted-foreground mt-0.5">Use password or private key below (one is required for SSH)</p>
              </label>
              <div className="text-sm text-card-foreground md:col-span-2">
                <div className="flex items-center justify-between gap-2 mb-1">
                  <span>SSH Private Key (optional)</span>
                  <input
                    type="file"
                    id="ssh-key-file"
                    className="hidden"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (!file) return;
                      const reader = new FileReader();
                      reader.onload = () => {
                        const text = typeof reader.result === 'string' ? reader.result : '';
                        setFormState((prev) => ({ ...prev, sshPrivateKey: text }));
                      };
                      reader.readAsText(file);
                      e.target.value = '';
                    }}
                  />
                  <label
                    htmlFor="ssh-key-file"
                    className="px-2.5 py-1.5 bg-muted hover:bg-muted/80 border border-border rounded text-xs font-medium text-card-foreground cursor-pointer flex items-center gap-1.5"
                  >
                    <span className="material-symbols-outlined text-[14px]">upload_file</span>
                    Upload file
                  </label>
                </div>
                <textarea
                  value={formState.sshPrivateKey}
                  onChange={(e) => setFormState((prev) => ({ ...prev, sshPrivateKey: e.target.value }))}
                  className="w-full px-3 py-2 bg-card border border-border rounded-lg font-mono text-xs min-h-[100px]"
                  placeholder={formState.id ? 'Leave empty to keep existing' : 'Paste private key (-----BEGIN ... -----END ...) or upload file above'}
                  rows={4}
                  spellCheck={false}
                />
                <p className="text-xs text-muted-foreground mt-0.5">Paste key or upload file (.pem, .key, .rsa, id_rsa...) — content will be read as text</p>
              </div>
            </>
          )}

          <label className="flex items-center gap-2 text-sm text-card-foreground">
            <input
              type="checkbox"
              checked={formState.isEnabled}
              onChange={(e) => setFormState((prev) => ({ ...prev, isEnabled: e.target.checked }))}
            />
            Enabled
          </label>
          <label className="flex items-center gap-2 text-sm text-card-foreground">
            <input
              type="checkbox"
              checked={formState.isDefault}
              onChange={(e) => setFormState((prev) => ({ ...prev, isDefault: e.target.checked }))}
            />
            Default environment
          </label>

          <div className="md:col-span-2 flex gap-2">
            <button
              onClick={() => void upsertEnvironment()}
              disabled={saving}
              className="px-3 py-2 bg-primary text-primary-foreground text-sm rounded-lg disabled:opacity-60"
            >
              {saving ? 'Saving...' : isEditing ? 'Update' : 'Create'}
            </button>
            <button
              onClick={resetForm}
              className="px-3 py-2 bg-muted hover:bg-muted/70 text-card-foreground text-sm rounded-lg"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {loadingProp ? (
        <p className="text-sm text-muted-foreground">Loading environments...</p>
      ) : environmentsProp.length === 0 ? (
        <p className="text-sm text-muted-foreground">No deployment environment configured.</p>
      ) : (
        <div className="space-y-3">
          {environmentsProp.map((env) => {
            const result = testResults[env.id];
            const working = Boolean(workingByEnvId[env.id]);

            return (
              <div key={env.id} className="p-4 border border-border rounded-lg bg-card/50 space-y-2">
                <div className="flex flex-col md:flex-row md:items-center justify-between gap-2">
                  <div>
                    <p className="font-medium text-card-foreground flex items-center gap-2">
                      {env.name}
                      <span className="text-xs px-2 py-0.5 rounded bg-muted text-muted-foreground">
                        {env.target_type}
                      </span>
                      {env.is_default && (
                        <span className="text-xs px-2 py-0.5 rounded bg-primary/10 text-primary">default</span>
                      )}
                      {!env.is_enabled && (
                        <span className="text-xs px-2 py-0.5 rounded bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300">
                          disabled
                        </span>
                      )}
                    </p>
                    <p className="text-xs text-muted-foreground mt-1">{env.deploy_path}</p>
                  </div>

                  <div className="flex flex-wrap gap-2">
                    <button
                      onClick={() => {
                        setFormState(toFormState(env));
                        setShowForm(true);
                      }}
                      className="px-2.5 py-1.5 text-xs border border-border rounded hover:bg-muted"
                    >
                      Edit
                    </button>
                    <button
                      onClick={() => void runTest(env, 'connection')}
                      disabled={working}
                      className="px-2.5 py-1.5 text-xs border border-border rounded hover:bg-muted disabled:opacity-60"
                    >
                      Test Connection
                    </button>
                    <button
                      onClick={() => void runTest(env, 'domain')}
                      disabled={working}
                      className="px-2.5 py-1.5 text-xs border border-border rounded hover:bg-muted disabled:opacity-60"
                    >
                      Test Domain
                    </button>
                    <button
                      onClick={() => void handleDelete(env)}
                      disabled={working}
                      className="px-2.5 py-1.5 text-xs border border-red-300 text-red-600 rounded hover:bg-red-50 dark:border-red-500/40 dark:text-red-300 dark:hover:bg-red-500/10 disabled:opacity-60"
                    >
                      Delete
                    </button>
                  </div>
                </div>

                {result && (
                  <div
                    className={`text-xs px-3 py-2 rounded border ${
                      result.success
                        ? 'border-green-200 bg-green-50 text-green-700 dark:border-green-500/30 dark:bg-green-500/10 dark:text-green-300'
                        : 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300'
                    }`}
                  >
                    <div className="space-y-1.5">
                      {result.checks.map((check) => (
                        <div key={check.step} className="flex flex-col gap-0.5">
                          <span className="font-medium">
                            {check.step}: <span className={check.status === 'pass' ? 'text-green-600 dark:text-green-400' : 'text-amber-600 dark:text-amber-400'}>{check.status}</span>
                          </span>
                          {check.message && (
                            <span className="text-muted-foreground pl-2 border-l-2 border-current/30">
                              {check.message}
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
