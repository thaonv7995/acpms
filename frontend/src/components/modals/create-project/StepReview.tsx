/**
 * StepReview - Step 4: Review & Create
 *
 * Final review before project creation:
 * - Summary of all selected options
 * - Configuration preview
 * - Edit links to go back to previous steps
 */

import { type ProjectType, getProjectTypeInfo } from '../../../api/templates';
import { TypeIconBadge } from './TypeIcon';
import { type ProjectConfig } from './StepConfigure';
import { type CreationMethod } from './StepSelectMethod';
import type { RefAttachment } from './ReferenceFilesUpload';

interface StepReviewProps {
  creationMethod: CreationMethod;
  projectType: ProjectType;
  config: ProjectConfig;
  repoUrl?: string;
  referenceAttachments?: RefAttachment[];
  onEditStep: (step: number) => void;
}

export function StepReview({
  creationMethod,
  projectType,
  config,
  repoUrl,
  referenceAttachments = [],
  onEditStep,
}: StepReviewProps) {
  const typeInfo = getProjectTypeInfo(projectType);
  const webSelections =
    projectType === 'web' && config.configMode === 'manual'
      ? config.stackSelections.filter((row) => row.stack.trim().length > 0)
      : [];

  return (
    <div className="space-y-6">
      <p className="text-sm text-slate-500 dark:text-slate-400">
        Review your project configuration before creating. Click any section to edit.
      </p>

      {/* Project Overview */}
      <ReviewSection title="Project Overview" onEdit={() => onEditStep(2)}>
        <div className="flex items-start gap-4">
          <TypeIconBadge type={projectType} size="lg" />
          <div className="flex-1">
            <h4 className="text-lg font-bold text-slate-900 dark:text-white">
              {config.name || 'Untitled Project'}
            </h4>
            <p className="text-sm text-slate-500 dark:text-slate-400 mt-1">
              {config.description || 'No description provided'}
            </p>
            <div className="flex items-center gap-4 mt-3">
              <Badge icon="category" label={typeInfo.label} />
              <Badge icon="visibility" label={getVisibilityLabel(config.visibility)} />
            </div>
          </div>
        </div>
      </ReviewSection>

      {/* Creation Method */}
      <ReviewSection title="Creation Method" onEdit={() => onEditStep(0)}>
        <div className="flex items-center gap-3">
          <MethodBadge method={creationMethod} />
          <div>
            <p className="text-sm font-medium text-slate-900 dark:text-white">
              {getMethodLabel(creationMethod)}
            </p>
            {creationMethod === 'gitlab' && repoUrl && (
              <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5 font-mono">
                {repoUrl}
              </p>
            )}
          </div>
        </div>
      </ReviewSection>

      {/* Technical Configuration */}
      <ReviewSection title="Technical Configuration" onEdit={() => onEditStep(2)}>
        <div className="grid grid-cols-2 gap-4">
          <ConfigItem
            label="Configuration Mode"
            value={config.configMode === 'ai' ? 'AI Architect' : 'Manual'}
            icon={config.configMode === 'ai' ? 'auto_fix' : 'tune'}
          />
          {config.configMode === 'manual' && projectType !== 'web' && config.techStack && (
            <ConfigItem
              label="Tech Stack"
              value={getTechStackLabel(config.techStack, typeInfo)}
              icon="code"
            />
          )}
          <ConfigItem
            label="Default Build"
            value={typeInfo.defaultBuildCommand}
            icon="build"
            mono
          />
          <ConfigItem
            label="Task Preview"
            value={typeInfo.supportsPreview ? 'Available' : 'Not Available'}
            icon="visibility"
          />
        </div>
        {webSelections.length > 0 && (
          <div className="mt-4 p-3 rounded-lg border border-border bg-muted">
            <p className="text-[10px] uppercase font-bold text-muted-foreground mb-2">
              Web Architecture Stack
            </p>
            <div className="flex flex-wrap gap-2">
              {webSelections.map((row, index) => (
                <span
                  key={`web-stack-${index}`}
                  className="inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-card border border-border text-xs text-card-foreground"
                >
                  <span className="font-bold text-primary">
                    {formatWebLayerLabel(row.layer)}:
                  </span>
                  <span>{formatStackValue(row.stack)}</span>
                </span>
              ))}
            </div>
          </div>
        )}
      </ReviewSection>

      {/* Reference Files */}
      {creationMethod === 'scratch' && referenceAttachments.length > 0 && (
        <ReviewSection title="Reference Files" onEdit={() => onEditStep(2)}>
          <div className="flex flex-wrap gap-2">
            {referenceAttachments
              .filter((a) => a.status === 'uploaded')
              .map((a) => (
                <span
                  key={a.id}
                  className="inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-muted text-xs text-card-foreground"
                >
                  <span className="material-symbols-outlined text-emerald-500 text-[14px]">
                    check_circle
                  </span>
                  {a.filename}
                </span>
              ))}
          </div>
        </ReviewSection>
      )}

      {/* Settings */}
      <ReviewSection title="Initial Settings" onEdit={() => onEditStep(2)}>
        <div className="space-y-2">
          <SettingItem
            label="Require Code Review"
            enabled={config.customSettings.requireReview}
          />
          <SettingItem
            label="Auto-create Init Task"
            enabled={config.customSettings.autoCreateInitTask}
          />
          {typeInfo.supportsPreview && (
            <SettingItem
              label="Enable Task Preview Delivery"
              enabled={config.customSettings.enablePreview}
            />
          )}
        </div>
      </ReviewSection>

      {/* Final Notice */}
      <div className="p-4 rounded-lg bg-blue-50 dark:bg-blue-500/20 border border-blue-200 dark:border-blue-500/30">
        <div className="flex items-start gap-3">
          <span className="material-symbols-outlined text-blue-500 text-lg mt-0.5">info</span>
          <div>
            <p className="text-sm text-blue-800 dark:text-blue-200">
              <strong>What happens next?</strong>
            </p>
            <ul className="text-xs text-blue-700 dark:text-blue-300 mt-1 space-y-1 list-disc list-inside">
              <li>
                {creationMethod === 'gitlab'
                  ? 'Your existing GitLab or GitHub repository will be connected'
                  : 'A new repository will be created and checked for GitOps access automatically'}
              </li>
              {creationMethod === 'gitlab' && (
                <li>If the repository is read-only, ACPMS can switch the project to a writable fork under your account.</li>
              )}
              {config.customSettings.autoCreateInitTask && (
                <li>An initialization task will be created for the AI agent</li>
              )}
              {creationMethod === 'scratch' && referenceAttachments.some((a) => a.status === 'uploaded') && (
                <li>Reference files will be available for the agent to read</li>
              )}
              {!config.customSettings.autoCreateInitTask && (
                <li>No initialization task will be created automatically</li>
              )}
              <li>You can start adding tasks and requirements immediately</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}

// Sub-components

interface ReviewSectionProps {
  title: string;
  onEdit: () => void;
  children: React.ReactNode;
}

function ReviewSection({ title, onEdit, children }: ReviewSectionProps) {
  return (
    <div className="p-4 rounded-lg bg-card border border-border">
      <div className="flex items-center justify-between mb-3">
        <h4 className="text-sm font-bold text-card-foreground">{title}</h4>
        <button
          onClick={onEdit}
          className="text-xs text-primary hover:text-blue-600 font-medium flex items-center gap-1"
        >
          <span className="material-symbols-outlined text-[14px]">edit</span>
          Edit
        </button>
      </div>
      {children}
    </div>
  );
}

interface BadgeProps {
  icon: string;
  label: string;
}

function Badge({ icon, label }: BadgeProps) {
  return (
    <span className="inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-muted text-xs text-card-foreground">
      <span className="material-symbols-outlined text-[14px]">{icon}</span>
      {label}
    </span>
  );
}

interface MethodBadgeProps {
  method: CreationMethod;
}

function MethodBadge({ method }: MethodBadgeProps) {
  const configs: Record<CreationMethod, { icon: string; color: string; bg: string }> = {
    scratch: { icon: 'add_circle', color: 'text-primary', bg: 'bg-primary/10' },
    gitlab: { icon: 'code', color: 'text-[#FC6D26]', bg: 'bg-[#FC6D26]/10' },
  };
  const config = configs[method];

  return (
    <div className={`size-10 rounded-full ${config.bg} flex items-center justify-center`}>
      <span className={`material-symbols-outlined ${config.color} text-xl`}>{config.icon}</span>
    </div>
  );
}

interface ConfigItemProps {
  label: string;
  value: string;
  icon: string;
  mono?: boolean;
}

function ConfigItem({ label, value, icon, mono }: ConfigItemProps) {
  return (
    <div className="flex items-start gap-2">
      <span className="material-symbols-outlined text-muted-foreground text-lg mt-0.5">
        {icon}
      </span>
      <div>
        <p className="text-[10px] uppercase font-bold text-muted-foreground">
          {label}
        </p>
        <p className={`text-sm text-card-foreground ${mono ? 'font-mono text-xs' : ''}`}>
          {value}
        </p>
      </div>
    </div>
  );
}

interface SettingItemProps {
  label: string;
  enabled: boolean;
}

function SettingItem({ label, enabled }: SettingItemProps) {
  return (
    <div className="flex items-center gap-2">
      <span
        className={`material-symbols-outlined text-lg ${
          enabled ? 'text-emerald-500' : 'text-muted-foreground'
        }`}
      >
        {enabled ? 'check_circle' : 'cancel'}
      </span>
      <span className="text-sm text-card-foreground">{label}</span>
    </div>
  );
}

// Helper functions

function getVisibilityLabel(visibility: string): string {
  const labels: Record<string, string> = {
    private: 'Private',
    internal: 'Internal',
    public: 'Public',
  };
  return labels[visibility] || visibility;
}

function getMethodLabel(method: CreationMethod): string {
  const labels: Record<CreationMethod, string> = {
    scratch: 'Create from Scratch',
    gitlab: 'Import from GitLab or GitHub',
  };
  return labels[method];
}

function getTechStackLabel(
  stackValue: string,
  typeInfo: ReturnType<typeof getProjectTypeInfo>
): string {
  const stack = typeInfo.defaultTechStacks.find((s) => s.value === stackValue);
  return stack?.name || stackValue;
}

function formatWebLayerLabel(layer: string): string {
  const labels: Record<string, string> = {
    frontend: 'Frontend',
    backend: 'Backend',
    database: 'Database',
    auth: 'Auth',
    cache: 'Cache',
    queue: 'Queue',
  };
  return labels[layer] || layer;
}

function formatStackValue(stackValue: string): string {
  return stackValue
    .split(/[-_]/)
    .filter((part) => part.length > 0)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

export default StepReview;
