/**
 * CreateProjectModal - Multi-step wizard for project creation
 *
 * Wizard Steps:
 * 1. Choose Creation Method (scratch, gitlab)
 * 2. Select Project Type (6 types)
 * 3. Configure Details (type-specific options)
 * 4. Review & Create
 *
 * Supports two creation flows:
 * - From Scratch: Empty repo with agent scaffolding
 * - Import from GitLab: Clone existing repository
 */

import { useState, useCallback, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import {
  createProject,
  importProjectCreateFork,
  importProjectFromGitLab,
  importProjectPreflight,
} from '../../api/projects';
import { type ProjectType } from '../../api/templates';
import { useDebouncedValue } from '../../hooks/useDebouncedValue';
import type {
  ImportProjectCreateForkResponse,
  ImportProjectPreflightResponse,
} from '../../types/repository';

// Step components
import { StepSelectMethod, type CreationMethod } from './create-project/StepSelectMethod';
import { StepSelectType } from './create-project/StepSelectType';
import { StepConfigure, type ProjectConfig } from './create-project/StepConfigure';
import { StepReview } from './create-project/StepReview';
import { GitLabImportForm } from './create-project/GitLabImportForm';
import { getReferenceKeys, type RefAttachment } from './create-project/ReferenceFilesUpload';
import { logger } from '@/lib/logger';

interface CreateProjectModalProps {
  isOpen: boolean;
  onClose: () => void;
}

// Wizard step definitions
type WizardStep = 'method' | 'type' | 'configure' | 'review' | 'gitlab';

const STEP_ORDER: WizardStep[] = ['method', 'type', 'configure', 'review'];

// Default project configuration
const defaultConfig: ProjectConfig = {
  name: '',
  description: '',
  techStack: '',
  stackSelections: [],
  visibility: 'private',
  configMode: 'ai',
  customSettings: {
    requireReview: true,
    autoCreateInitTask: true,
    enablePreview: false,
  },
};

export function CreateProjectModal({ isOpen, onClose }: CreateProjectModalProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // Wizard state
  const [currentStep, setCurrentStep] = useState<WizardStep>('method');
  const [creationMethod, setCreationMethod] = useState<CreationMethod | null>(null);
  const [projectType, setProjectType] = useState<ProjectType | null>(null);
  const [config, setConfig] = useState<ProjectConfig>(defaultConfig);

  // GitLab import state
  const [repoUrl, setRepoUrl] = useState('');
  const [upstreamRepoUrl, setUpstreamRepoUrl] = useState('');
  const [repoPreflight, setRepoPreflight] = useState<ImportProjectPreflightResponse | null>(null);
  const [repoPreflightUrl, setRepoPreflightUrl] = useState('');
  const [repoPreflightLoading, setRepoPreflightLoading] = useState(false);
  const [repoPreflightError, setRepoPreflightError] = useState<string | null>(null);
  const [repoForkPending, setRepoForkPending] = useState(false);
  const debouncedRepoUrl = useDebouncedValue(repoUrl.trim(), 450);

  // Reference files (From Scratch only)
  const [referenceAttachments, setReferenceAttachments] = useState<RefAttachment[]>([]);

  // UI state
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Config change handler - must be defined before early return
  const handleConfigChange = useCallback((newConfig: ProjectConfig) => {
    setConfig(newConfig);
  }, []);

  // Preflight effect: must run on every render (before early return) to satisfy Rules of Hooks
  useEffect(() => {
    if (currentStep !== 'gitlab') {
      setRepoPreflight(null);
      setRepoPreflightUrl('');
      setRepoPreflightLoading(false);
      setRepoPreflightError(null);
      setRepoForkPending(false);
      return;
    }

    if (!debouncedRepoUrl) {
      setRepoPreflight(null);
      setRepoPreflightUrl('');
      setRepoPreflightLoading(false);
      setRepoPreflightError(null);
      return;
    }

    let cancelled = false;
    setRepoPreflightLoading(true);
    setRepoPreflightError(null);

    void importProjectPreflight({
      repository_url: debouncedRepoUrl,
      upstream_repository_url: upstreamRepoUrl.trim() || undefined,
    })
      .then((response) => {
        if (cancelled) return;
        setRepoPreflight(response);
        setRepoPreflightUrl(debouncedRepoUrl);
      })
      .catch((err) => {
        if (cancelled) return;
        setRepoPreflight(null);
        setRepoPreflightUrl('');
        setRepoPreflightError(err instanceof Error ? err.message : 'Failed to check repository access');
      })
      .finally(() => {
        if (!cancelled) {
          setRepoPreflightLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [currentStep, debouncedRepoUrl, upstreamRepoUrl]);

  const handleRepoUrlChange = useCallback((url: string) => {
    setRepoUrl(url);
    setUpstreamRepoUrl('');
    setRepoForkPending(false);
    setRepoPreflight(null);
    setRepoPreflightUrl('');
    setRepoPreflightError(null);
  }, []);

  const handleCreateForkForImport = useCallback(async () => {
    const currentRepoUrl = repoUrl.trim();
    if (!currentRepoUrl) {
      setRepoPreflightError('Repository URL is required before creating a fork.');
      return;
    }

    setRepoForkPending(true);
    setRepoPreflightError(null);

    try {
      const response: ImportProjectCreateForkResponse = await importProjectCreateFork({
        repository_url: currentRepoUrl,
      });
      setUpstreamRepoUrl(response.upstream_repository_url);
      setRepoUrl(response.fork_repository_url);
      setRepoPreflight({
        repository_context: response.repository_context,
        recommended_action: response.recommended_action,
        warnings: response.warnings,
      });
      setRepoPreflightUrl(response.fork_repository_url);
    } catch (err) {
      setRepoPreflightError(
        err instanceof Error ? err.message : 'Failed to create writable fork for import'
      );
    } finally {
      setRepoForkPending(false);
    }
  }, [repoUrl]);

  // Early return AFTER all hooks (required to avoid "Rendered more hooks than during the previous render")
  if (!isOpen) return null;

  // Reset wizard to initial state
  const resetWizard = () => {
    setCurrentStep('method');
    setCreationMethod(null);
    setProjectType(null);
    setConfig(defaultConfig);
    setRepoUrl('');
    setUpstreamRepoUrl('');
    setRepoPreflight(null);
    setRepoPreflightUrl('');
    setRepoPreflightLoading(false);
    setRepoPreflightError(null);
    setRepoForkPending(false);
    setReferenceAttachments([]);
    setIsCreating(false);
    setError(null);
  };

  const handleClose = () => {
    resetWizard();
    onClose();
  };

  // Navigation handlers
  const handleSelectMethod = (method: CreationMethod) => {
    setCreationMethod(method);
    setError(null);

    if (method === 'gitlab') {
      setCurrentStep('gitlab');
    } else {
      setCurrentStep('type');
    }
  };

  const handleSelectType = (type: ProjectType) => {
    setProjectType(type);
    setCurrentStep('configure');
  };

  const handleBack = () => {
    setError(null);

    switch (currentStep) {
      case 'type':
        setCurrentStep('method');
        break;
      case 'configure':
        if (creationMethod === 'gitlab') {
          setCurrentStep('gitlab');
        } else {
          setCurrentStep('type');
        }
        break;
      case 'review':
        setCurrentStep('configure');
        break;
      case 'gitlab':
        setCurrentStep('method');
        break;
    }
  };

  const handleNext = () => {
    setError(null);

    switch (currentStep) {
      case 'type':
        if (projectType) {
          setCurrentStep('configure');
        }
        break;
      case 'configure':
        if (config.name.trim()) {
          setCurrentStep('review');
        } else {
          setError('Project name is required');
        }
        break;
      case 'gitlab':
        // GitLab import: no type/configure/review — import directly
        if (config.name.trim() && repoUrl.trim()) {
          handleCreate();
        } else {
          setError('Project name and repository URL are required');
        }
        break;
    }
  };

  const handleEditStep = (stepIndex: number) => {
    const step = STEP_ORDER[stepIndex];
    if (step) {
      setCurrentStep(step);
    }
  };

  // Create project
  const handleCreate = async () => {
    setIsCreating(true);
    setError(null);

    try {
      if (creationMethod !== 'gitlab' && !projectType) {
        setError('Project type is required');
        setIsCreating(false);
        return;
      }

      let project;
      const preferredTechStack = projectType ? buildPreferredTechStack(projectType, config) : undefined;
      const stackSelections = projectType ? buildStackSelections(projectType, config) : undefined;

      if (creationMethod === 'gitlab') {
        // GitLab Import flow
        const importResponse = await importProjectFromGitLab({
          name: config.name.trim(),
          repository_url: repoUrl.trim(),
          upstream_repository_url: upstreamRepoUrl.trim() || undefined,
          description: config.description.trim() || undefined,
          require_review: config.customSettings.requireReview,
          project_type: undefined, // Auto-detect from repo after clone
          auto_create_init_task: config.customSettings.autoCreateInitTask,
          preview_enabled: config.customSettings.enablePreview,
        });
        project = importResponse.project;
      } else {
        // From Scratch flow
        const referenceKeys = getReferenceKeys(referenceAttachments);
        project = await createProject({
          name: config.name.trim(),
          description: config.description.trim() || undefined,
          create_from_scratch: true,
          visibility: config.visibility,
          tech_stack: preferredTechStack,
          stack_selections: stackSelections,
          require_review: config.customSettings.requireReview,
          auto_create_init_task: config.customSettings.autoCreateInitTask,
          project_type: projectType!,
          preview_enabled: config.customSettings.enablePreview,
          reference_keys: referenceKeys.length > 0 ? referenceKeys : undefined,
        });
      }

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['/api/v1/projects'] }),
        queryClient.invalidateQueries({ queryKey: ['/api/v1/dashboard'] }),
      ]);

      navigate(`/projects/${project.id}`);
      handleClose();
    } catch (err) {
      logger.error('Failed to create project:', err);
      setError(err instanceof Error ? err.message : 'Failed to create project');
      setIsCreating(false);
    }
  };

  // Get current step info
  const getStepInfo = () => {
    switch (currentStep) {
      case 'method':
        return { title: 'Start a Project', subtitle: 'Choose how you want to begin.' };
      case 'type':
        return { title: 'Select Project Type', subtitle: 'What are we building today?' };
      case 'configure':
        return { title: 'Configure Project', subtitle: 'Set up your project details.' };
      case 'review':
        return { title: 'Review & Create', subtitle: 'Confirm your project settings.' };
      case 'gitlab':
        return { title: 'Import from GitLab or GitHub', subtitle: 'Connect your existing repository.' };
    }
  };

  const stepInfo = getStepInfo();
  const stepIndex = STEP_ORDER.indexOf(currentStep);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-[2px] transition-opacity"
        onClick={handleClose}
      />
      <div className="relative w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="px-6 py-5 border-b border-border bg-muted">
          <div className="flex justify-between items-start">
            <div>
              <h2 className="text-xl font-bold text-card-foreground">
                {stepInfo.title}
              </h2>
              <p className="text-sm text-muted-foreground">{stepInfo.subtitle}</p>
            </div>
            <button
              onClick={handleClose}
              className="text-muted-foreground hover:text-card-foreground transition-colors"
            >
              <span className="material-symbols-outlined">close</span>
            </button>
          </div>

          {/* Step indicator */}
          {stepIndex >= 0 && currentStep !== 'gitlab' && (
            <StepIndicator currentStep={stepIndex} totalSteps={4} />
          )}
        </div>

        {/* Error Message */}
        {error && (
          <div className="mx-6 mt-4 p-3 bg-red-50 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 rounded-lg flex items-start gap-2">
            <span className="material-symbols-outlined text-red-600 dark:text-red-400 text-lg">
              error
            </span>
            <p className="text-sm text-red-800 dark:text-red-200 flex-1">{error}</p>
          </div>
        )}

        {/* Body */}
        <div className="p-6 overflow-y-auto flex-1">
          {currentStep === 'method' && <StepSelectMethod onSelectMethod={handleSelectMethod} />}

          {currentStep === 'type' && (
            <StepSelectType selectedType={projectType} onSelectType={handleSelectType} />
          )}

          {currentStep === 'configure' && projectType && (
            <StepConfigure
              projectType={projectType}
              config={config}
              onConfigChange={handleConfigChange}
              creationMethod={creationMethod ?? undefined}
              referenceAttachments={referenceAttachments}
              onReferenceAttachmentsChange={(updater) =>
                setReferenceAttachments((prev) =>
                  typeof updater === 'function' ? updater(prev) : updater
                )
              }
            />
          )}

          {currentStep === 'review' && projectType && creationMethod && (
            <StepReview
              creationMethod={creationMethod}
              projectType={projectType}
              config={config}
              repoUrl={creationMethod === 'gitlab' ? repoUrl : undefined}
              referenceAttachments={referenceAttachments}
              onEditStep={handleEditStep}
            />
          )}

          {currentStep === 'gitlab' && (
            <GitLabImportForm
              projectName={config.name}
              repoUrl={repoUrl}
              upstreamRepoUrl={upstreamRepoUrl}
              preflight={repoPreflight}
              preflightLoading={repoPreflightLoading}
              preflightError={repoPreflightError}
              forkPending={repoForkPending}
              onProjectNameChange={(name) => setConfig((prev) => ({ ...prev, name }))}
              onRepoUrlChange={handleRepoUrlChange}
              onCreateFork={handleCreateForkForImport}
            />
          )}
        </div>

        {/* Footer */}
        <WizardFooter
          currentStep={currentStep}
          canGoBack={currentStep !== 'method'}
          canGoNext={canProceed(
            currentStep,
            projectType,
            config,
            repoUrl,
            repoPreflight,
            repoPreflightUrl,
            repoPreflightLoading,
            repoPreflightError,
            repoForkPending
          )}
          isCreating={isCreating}
          onBack={handleBack}
          onNext={handleNext}
          onCreate={handleCreate}
        />
      </div>
    </div>
  );
}

// Step indicator component
interface StepIndicatorProps {
  currentStep: number;
  totalSteps: number;
}

function StepIndicator({ currentStep, totalSteps }: StepIndicatorProps) {
  const steps = ['Method', 'Type', 'Configure', 'Review'];

  return (
    <div className="flex items-center gap-2 mt-4">
      {steps.map((label, index) => (
        <div key={label} className="flex items-center">
          <div
            className={`flex items-center justify-center size-6 rounded-full text-xs font-bold ${
              index < currentStep
                ? 'bg-primary text-primary-foreground'
                : index === currentStep
                ? 'bg-primary/20 text-primary border-2 border-primary'
                : 'bg-slate-200 dark:bg-slate-700 text-slate-500'
            }`}
          >
            {index < currentStep ? (
              <span className="material-symbols-outlined text-[14px]">check</span>
            ) : (
              index + 1
            )}
          </div>
          <span
            className={`ml-1.5 text-xs font-medium hidden sm:inline ${
              index <= currentStep
                ? 'text-slate-900 dark:text-white'
                : 'text-slate-400 dark:text-slate-500'
            }`}
          >
            {label}
          </span>
          {index < totalSteps - 1 && (
            <div
              className={`w-8 h-0.5 mx-2 ${
                index < currentStep ? 'bg-primary' : 'bg-slate-200 dark:bg-slate-700'
              }`}
            />
          )}
        </div>
      ))}
    </div>
  );
}

// Footer component
interface WizardFooterProps {
  currentStep: WizardStep;
  canGoBack: boolean;
  canGoNext: boolean;
  isCreating: boolean;
  onBack: () => void;
  onNext: () => void;
  onCreate: () => void;
}

function WizardFooter({
  currentStep,
  canGoBack,
  canGoNext,
  isCreating,
  onBack,
  onNext,
  onCreate,
}: WizardFooterProps) {
  // Don't show footer for method step
  if (currentStep === 'method') {
    return null;
  }

  const isReviewStep = currentStep === 'review';
  const isGitLabStep = currentStep === 'gitlab';

  return (
    <div className="px-6 py-4 border-t border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-[#161b22] flex justify-between items-center">
      <button
        onClick={onBack}
        disabled={!canGoBack || isCreating}
        className="text-sm font-bold text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white transition-colors flex items-center gap-1 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <span className="material-symbols-outlined text-[18px]">arrow_back</span>
        Back
      </button>

      {isReviewStep ? (
        <button
          onClick={onCreate}
          disabled={isCreating}
          className="px-6 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isCreating ? (
            <>
              <span className="inline-block w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
              Creating...
            </>
          ) : (
            <>
              Create Project
              <span className="material-symbols-outlined text-[18px]">rocket_launch</span>
            </>
          )}
        </button>
      ) : (
        <button
          onClick={onNext}
          disabled={!canGoNext || isCreating}
          className="px-6 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isGitLabStep ? (
            isCreating ? (
              <>
                <span className="inline-block w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                Importing...
              </>
            ) : (
              <>
                Import Project
                <span className="material-symbols-outlined text-[18px]">download</span>
              </>
            )
          ) : (
            <>
              Next
              <span className="material-symbols-outlined text-[18px]">arrow_forward</span>
            </>
          )}
        </button>
      )}
    </div>
  );
}

// Helper to determine if user can proceed to next step
function canProceed(
  step: WizardStep,
  projectType: ProjectType | null,
  config: ProjectConfig,
  repoUrl: string,
  repoPreflight: ImportProjectPreflightResponse | null,
  repoPreflightUrl: string,
  repoPreflightLoading: boolean,
  repoPreflightError: string | null,
  repoForkPending: boolean
): boolean {
  switch (step) {
    case 'type':
      return projectType !== null;
    case 'configure':
      return config.name.trim().length > 0;
    case 'gitlab':
      return (
        config.name.trim().length > 0 &&
        repoUrl.trim().length > 0 &&
        !repoPreflightLoading &&
        !repoForkPending &&
        !repoPreflightError &&
        repoPreflight !== null &&
        repoPreflightUrl === repoUrl.trim()
      );
    default:
      return true;
  }
}

function buildPreferredTechStack(
  projectType: ProjectType,
  config: ProjectConfig
): string | undefined {
  if (projectType === 'web' && config.configMode === 'manual') {
    const selectedRows = config.stackSelections.filter((row) => row.stack.trim().length > 0);
    if (selectedRows.length > 0) {
      return selectedRows
        .map((row) => `${row.layer}:${row.stack}`)
        .join(' | ');
    }
  }

  const singleStack = config.techStack.trim();
  return singleStack || undefined;
}

function buildStackSelections(
  projectType: ProjectType,
  config: ProjectConfig
):
  | Array<{
      layer: 'frontend' | 'backend' | 'database' | 'auth' | 'cache' | 'queue';
      stack: string;
    }>
  | undefined {
  if (projectType !== 'web' || config.configMode !== 'manual') {
    return undefined;
  }

  const selections = config.stackSelections
    .map((row) => ({
      layer: row.layer,
      stack: row.stack.trim(),
    }))
    .filter((row) => row.stack.length > 0);

  return selections.length > 0 ? selections : undefined;
}

export default CreateProjectModal;
