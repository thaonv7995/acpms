/**
 * StepConfigure - Step 3: Configure Project Details
 *
 * Type-specific configuration:
 * - Project name and description
 * - Tech stack selection (based on project type)
 * - Visibility settings
 * - Initial settings toggles (AI Architect vs Manual)
 */

import { useState, useEffect, useMemo, useRef } from 'react';
import { type ProjectType, type TechStack, getProjectTypeInfo } from '../../../api/templates';
import { TypeIconBadge } from './TypeIcon';
import { ReferenceFilesUpload, type RefAttachment } from './ReferenceFilesUpload';

export type ConfigMode = 'ai' | 'manual';
export type WebStackLayer =
  | 'frontend'
  | 'backend'
  | 'database'
  | 'auth'
  | 'cache'
  | 'queue';

export interface WebStackSelection {
  layer: WebStackLayer;
  stack: string;
}

interface LayerOption {
  value: WebStackLayer;
  label: string;
}

const WEB_STACK_LAYER_OPTIONS: LayerOption[] = [
  { value: 'frontend', label: 'Frontend' },
  { value: 'backend', label: 'Backend' },
  { value: 'database', label: 'Database' },
  { value: 'auth', label: 'Authentication' },
  { value: 'cache', label: 'Cache' },
  { value: 'queue', label: 'Queue / Messaging' },
];

const WEB_STACK_OPTIONS: Record<WebStackLayer, TechStack[]> = {
  frontend: [
    { name: 'React + Vite', value: 'react-vite' },
    { name: 'Next.js', value: 'nextjs' },
    { name: 'Vue.js', value: 'vuejs' },
    { name: 'Nuxt 3', value: 'nuxt3' },
    { name: 'SvelteKit', value: 'sveltekit' },
    { name: 'Angular', value: 'angular' },
    { name: 'Remix', value: 'remix' },
    { name: 'Astro', value: 'astro' },
    { name: 'Qwik', value: 'qwik' },
  ],
  backend: [
    { name: 'NestJS', value: 'nestjs' },
    { name: 'Express.js', value: 'express' },
    { name: 'Fastify', value: 'fastify' },
    { name: 'Hono', value: 'hono' },
    { name: 'FastAPI', value: 'fastapi' },
    { name: 'Django REST', value: 'django-rest' },
    { name: 'Spring Boot', value: 'spring-boot' },
    { name: 'ASP.NET Core', value: 'aspnet-core' },
    { name: 'Laravel API', value: 'laravel-api' },
    { name: 'Axum', value: 'axum' },
    { name: 'Gin', value: 'gin' },
    { name: 'Fiber', value: 'fiber' },
  ],
  database: [
    { name: 'PostgreSQL', value: 'postgresql' },
    { name: 'MySQL', value: 'mysql' },
    { name: 'MariaDB', value: 'mariadb' },
    { name: 'SQLite', value: 'sqlite' },
    { name: 'MongoDB', value: 'mongodb' },
    { name: 'Redis', value: 'redis' },
    { name: 'CockroachDB', value: 'cockroachdb' },
    { name: 'DynamoDB', value: 'dynamodb' },
    { name: 'Firebase Firestore', value: 'firebase-firestore' },
    { name: 'Supabase Postgres', value: 'supabase-postgres' },
  ],
  auth: [
    { name: 'Auth0', value: 'auth0' },
    { name: 'Clerk', value: 'clerk' },
    { name: 'NextAuth / Auth.js', value: 'nextauth' },
    { name: 'Keycloak', value: 'keycloak' },
    { name: 'Supabase Auth', value: 'supabase-auth' },
    { name: 'Firebase Auth', value: 'firebase-auth' },
    { name: 'JWT + Passport', value: 'jwt-passport' },
  ],
  cache: [
    { name: 'Redis Cache', value: 'redis-cache' },
    { name: 'Memcached', value: 'memcached' },
    { name: 'Cloudflare KV', value: 'cloudflare-kv' },
  ],
  queue: [
    { name: 'RabbitMQ', value: 'rabbitmq' },
    { name: 'Kafka', value: 'kafka' },
    { name: 'BullMQ', value: 'bullmq' },
    { name: 'AWS SQS', value: 'aws-sqs' },
    { name: 'NATS', value: 'nats' },
  ],
};

const createDefaultWebStackSelection = (): WebStackSelection => ({
  layer: 'frontend',
  stack: '',
});

export interface ProjectConfig {
  name: string;
  description: string;
  techStack: string;
  stackSelections: WebStackSelection[];
  visibility: 'private' | 'public' | 'internal';
  configMode: ConfigMode;
  customSettings: {
    requireReview: boolean;
    autoCreateInitTask: boolean;
    enablePreview: boolean;
  };
}

interface StepConfigureProps {
  projectType: ProjectType;
  config: ProjectConfig;
  onConfigChange: (config: ProjectConfig) => void;
  creationMethod?: 'scratch' | 'gitlab';
  referenceAttachments?: RefAttachment[];
  onReferenceAttachmentsChange?: (updater: RefAttachment[] | ((prev: RefAttachment[]) => RefAttachment[])) => void;
}

export function StepConfigure({
  projectType,
  config,
  onConfigChange,
  creationMethod,
  referenceAttachments = [],
  onReferenceAttachmentsChange,
}: StepConfigureProps) {
  const typeInfo = getProjectTypeInfo(projectType);
  const [localConfig, setLocalConfig] = useState<ProjectConfig>(config);
  const [stackSearch, setStackSearch] = useState('');
  const [isTechStackOpen, setIsTechStackOpen] = useState(false);
  const techStackDropdownRef = useRef<HTMLDivElement>(null);

  const selectedTechStack = useMemo(
    () =>
      typeInfo.defaultTechStacks.find((stack) => stack.value === localConfig.techStack) || null,
    [typeInfo.defaultTechStacks, localConfig.techStack]
  );

  const filteredTechStacks = useMemo(() => {
    const query = stackSearch.trim().toLowerCase();
    if (!query) {
      return typeInfo.defaultTechStacks;
    }

    return typeInfo.defaultTechStacks.filter((stack) => {
      const label = stack.name.toLowerCase();
      const value = stack.value.toLowerCase();
      return label.includes(query) || value.includes(query);
    });
  }, [stackSearch, typeInfo.defaultTechStacks]);

  const isWebManualMode = projectType === 'web' && localConfig.configMode === 'manual';

  // Sync local config back to parent
  useEffect(() => {
    onConfigChange(localConfig);
  }, [localConfig, onConfigChange]);

  useEffect(() => {
    setStackSearch('');
    setIsTechStackOpen(false);
  }, [projectType, localConfig.configMode]);

  useEffect(() => {
    if (!isTechStackOpen) {
      return;
    }

    const handleClickOutside = (event: MouseEvent) => {
      if (
        techStackDropdownRef.current &&
        !techStackDropdownRef.current.contains(event.target as Node)
      ) {
        setIsTechStackOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isTechStackOpen]);

  useEffect(() => {
    if (!isWebManualMode || localConfig.stackSelections.length > 0) {
      return;
    }
    setLocalConfig((prev) => ({
      ...prev,
      stackSelections: [createDefaultWebStackSelection()],
    }));
  }, [isWebManualMode, localConfig.stackSelections.length]);

  const updateConfig = (updates: Partial<ProjectConfig>) => {
    setLocalConfig((prev) => ({ ...prev, ...updates }));
  };

  const updateCustomSettings = (updates: Partial<ProjectConfig['customSettings']>) => {
    setLocalConfig((prev) => ({
      ...prev,
      customSettings: { ...prev.customSettings, ...updates },
    }));
  };

  const addWebStackRow = () => {
    setLocalConfig((prev) => ({
      ...prev,
      stackSelections: [...prev.stackSelections, createDefaultWebStackSelection()],
    }));
  };

  const removeWebStackRow = (index: number) => {
    setLocalConfig((prev) => {
      const nextSelections = prev.stackSelections.filter((_, idx) => idx !== index);
      return {
        ...prev,
        stackSelections:
          nextSelections.length > 0 ? nextSelections : [createDefaultWebStackSelection()],
      };
    });
  };

  const updateWebStackRow = (
    index: number,
    updates: Partial<WebStackSelection>
  ) => {
    setLocalConfig((prev) => {
      const nextSelections = [...prev.stackSelections];
      const current = nextSelections[index] || createDefaultWebStackSelection();
      let nextRow = { ...current, ...updates };

      if (updates.layer && updates.layer !== current.layer) {
        nextRow = {
          ...nextRow,
          stack: '',
        };
      }

      nextSelections[index] = nextRow;
      return {
        ...prev,
        stackSelections: nextSelections,
      };
    });
  };

  return (
    <div className="space-y-6">
      {/* Type indicator */}
      <div className="flex items-center gap-3 p-3 rounded-lg bg-muted border border-border">
        <TypeIconBadge type={projectType} size="sm" />
        <div>
          <p className="text-sm font-bold text-card-foreground">{typeInfo.label}</p>
          <p className="text-xs text-muted-foreground">{typeInfo.description}</p>
        </div>
      </div>

      {/* Project Name */}
      <div>
        <label className="block text-sm font-bold text-card-foreground mb-1.5">
          Project Name <span className="text-red-500">*</span>
        </label>
        <input
          type="text"
          value={localConfig.name}
          onChange={(e) => updateConfig({ name: e.target.value })}
          placeholder="my-awesome-project"
          maxLength={64}
          className="w-full bg-muted border border-border rounded-lg py-2.5 px-4 text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
        />
        <p className="mt-1 text-xs text-muted-foreground">
          Repository name will be: <code className="font-mono bg-muted px-1 rounded">{slugifyPreview(localConfig.name) || '(empty)'}</code>
        </p>
        {localConfig.name.trim() && !slugifyPreview(localConfig.name) && (
          <p className="mt-1 text-xs text-amber-600 dark:text-amber-400">
            Use only letters, numbers, spaces, or hyphens for a valid repository name
          </p>
        )}
      </div>

      {/* Description */}
      <div>
        <label className="block text-sm font-bold text-card-foreground mb-1.5">
          Description
        </label>
        <textarea
          value={localConfig.description}
          onChange={(e) => updateConfig({ description: e.target.value })}
          placeholder="A brief description of your project..."
          rows={3}
          className="w-full bg-muted border border-border rounded-lg py-2.5 px-4 text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground resize-none"
        />
      </div>

      {/* Configuration Mode Toggle */}
      <div>
        <label className="block text-sm font-bold text-card-foreground mb-2">
          Configuration Mode
        </label>
        <div className="flex bg-muted p-1 rounded-lg w-fit">
          <button
            onClick={() => updateConfig({ configMode: 'ai' })}
            className={`px-4 py-2 rounded-md text-sm font-bold flex items-center gap-2 transition-all ${
              localConfig.configMode === 'ai'
                ? 'bg-card text-primary shadow-sm'
                : 'text-muted-foreground hover:text-card-foreground'
            }`}
          >
            <span className="material-symbols-outlined text-[18px]">auto_fix</span>
            AI Architect
          </button>
          <button
            onClick={() => updateConfig({ configMode: 'manual' })}
            className={`px-4 py-2 rounded-md text-sm font-bold flex items-center gap-2 transition-all ${
              localConfig.configMode === 'manual'
                ? 'bg-card text-primary shadow-sm'
                : 'text-muted-foreground hover:text-card-foreground'
            }`}
          >
            <span className="material-symbols-outlined text-[18px]">tune</span>
            Manual Config
          </button>
        </div>
      </div>

      {/* Tech Stack Selection (Manual mode) */}
      {localConfig.configMode === 'manual' && !isWebManualMode && (
        <div>
          <label className="block text-sm font-bold text-card-foreground mb-1.5">
            Tech Stack
          </label>
          <div className="relative" ref={techStackDropdownRef}>
            <button
              type="button"
              onClick={() => setIsTechStackOpen((prev) => !prev)}
              className={`w-full flex items-center justify-between rounded-lg border px-4 py-2.5 text-left transition-colors ${
                isTechStackOpen
                  ? 'border-primary ring-1 ring-primary bg-card'
                  : 'border-border bg-muted hover:border-primary/50'
              }`}
            >
              <div className="min-w-0">
                <p className={`text-sm font-medium ${selectedTechStack ? 'text-card-foreground' : 'text-muted-foreground'}`}>
                  {selectedTechStack ? selectedTechStack.name : 'Select a tech stack...'}
                </p>
                {selectedTechStack && (
                  <p className="text-xs text-muted-foreground truncate">
                    {selectedTechStack.value}
                  </p>
                )}
              </div>
              <div className="flex items-center gap-2">
                {selectedTechStack && (
                  <span
                    role="button"
                    tabIndex={0}
                    onClick={(event) => {
                      event.stopPropagation();
                      setStackSearch('');
                      updateConfig({ techStack: '' });
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        event.stopPropagation();
                        setStackSearch('');
                        updateConfig({ techStack: '' });
                      }
                    }}
                    className="material-symbols-outlined text-[18px] text-muted-foreground hover:text-card-foreground"
                  >
                    close
                  </span>
                )}
                <span className="material-symbols-outlined text-[20px] text-muted-foreground">
                  {isTechStackOpen ? 'expand_less' : 'expand_more'}
                </span>
              </div>
            </button>

            {isTechStackOpen && (
              <div className="absolute z-30 mt-2 w-full rounded-lg border border-border bg-card shadow-2xl overflow-hidden">
                <div className="p-2 border-b border-border">
                  <div className="relative">
                    <span className="material-symbols-outlined text-[18px] text-muted-foreground absolute left-2.5 top-1/2 -translate-y-1/2">
                      search
                    </span>
                    <input
                      value={stackSearch}
                      onChange={(event) => setStackSearch(event.target.value)}
                      placeholder="Search technologies..."
                      className="w-full bg-muted border border-border rounded-md py-2 pl-9 pr-3 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                    />
                  </div>
                </div>

                <div className="max-h-64 overflow-y-auto py-1">
                  {filteredTechStacks.length > 0 ? (
                    filteredTechStacks.map((stack) => (
                      <TechStackOption
                        key={stack.value}
                        stack={stack}
                        selected={stack.value === localConfig.techStack}
                        onSelect={(value) => {
                          updateConfig({ techStack: value });
                          setIsTechStackOpen(false);
                          setStackSearch('');
                        }}
                      />
                    ))
                  ) : (
                    <p className="px-4 py-3 text-sm text-muted-foreground">
                      No matching tech stack. Try another keyword.
                    </p>
                  )}
                </div>
              </div>
            )}
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            Search by framework name or keyword (for example: react, tauri, grpc, fastapi).
          </p>
        </div>
      )}

      {/* Web Architecture Stack Selection (Manual mode) */}
      {isWebManualMode && (
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <label className="block text-sm font-bold text-card-foreground">
              Architecture Stack
            </label>
            <button
              type="button"
              onClick={addWebStackRow}
              className="inline-flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-bold text-card-foreground border border-border bg-card hover:border-primary/60 hover:text-primary hover:bg-primary/10 transition-colors shadow-sm"
            >
              <span className="material-symbols-outlined text-[16px] text-primary">add</span>
              Add stack row
            </button>
          </div>

          <div className="space-y-2">
            {localConfig.stackSelections.map((selection, index) => (
              <div
                key={`stack-row-${index}`}
                className="grid grid-cols-1 md:grid-cols-[180px_1fr_auto] gap-2 items-center p-2 rounded-lg border border-border bg-muted"
              >
                <select
                  value={selection.layer}
                  onChange={(event) =>
                    updateWebStackRow(index, {
                      layer: event.target.value as WebStackLayer,
                    })
                  }
                  className="w-full bg-card border border-border rounded-md py-2 px-3 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                >
                  {WEB_STACK_LAYER_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>

                <select
                  value={selection.stack}
                  onChange={(event) =>
                    updateWebStackRow(index, {
                      stack: event.target.value,
                    })
                  }
                  className="w-full bg-card border border-border rounded-md py-2 px-3 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                >
                  <option value="">Select stack...</option>
                  {(WEB_STACK_OPTIONS[selection.layer] || []).map((stackOption) => (
                    <option key={`${selection.layer}-${stackOption.value}`} value={stackOption.value}>
                      {stackOption.name}
                    </option>
                  ))}
                </select>

                <button
                  type="button"
                  onClick={() => removeWebStackRow(index)}
                  className="inline-flex items-center justify-center size-9 rounded-md border border-border text-muted-foreground hover:text-red-500 hover:border-red-400/50 transition-colors"
                  title="Remove row"
                >
                  <span className="material-symbols-outlined text-[18px]">delete</span>
                </button>
              </div>
            ))}
          </div>

          <p className="text-xs text-muted-foreground">
            Add multiple rows to combine frontend, backend, database, and infrastructure choices for this web app.
          </p>
        </div>
      )}

      {/* AI Description (AI mode) */}
      {localConfig.configMode === 'ai' && (
        <div>
          <label className="block text-sm font-bold text-card-foreground mb-2">
            Describe your project requirements
          </label>
          <div className="relative">
            <textarea
              value={localConfig.description}
              onChange={(e) => updateConfig({ description: e.target.value })}
              placeholder={getAIPlaceholder(projectType)}
              rows={4}
              className="w-full bg-muted border border-border rounded-lg p-4 text-sm text-card-foreground focus:ring-primary focus:border-primary resize-none placeholder-muted-foreground"
            />
            <div className="absolute bottom-3 right-3 flex items-center gap-2 text-xs text-muted-foreground bg-card px-2 py-1 rounded border border-border shadow-sm">
              <span className="material-symbols-outlined text-[14px] text-primary">smart_toy</span>
              AI will suggest the best stack
            </div>
          </div>
        </div>
      )}

      {/* Visibility */}
      <div>
        <label className="block text-sm font-bold text-card-foreground mb-1.5">
          GitLab Visibility
        </label>
        <select
          value={localConfig.visibility}
          onChange={(e) => updateConfig({ visibility: e.target.value as ProjectConfig['visibility'] })}
          className="w-full bg-muted border border-border rounded-lg py-2.5 px-4 text-card-foreground focus:ring-primary focus:border-primary"
        >
          <option value="private">Private - Only accessible to project members</option>
          <option value="internal">Internal - Accessible to all logged-in users</option>
          <option value="public">Public - Accessible to everyone</option>
        </select>
      </div>

      {/* Reference Files (From Scratch only) */}
      {creationMethod === 'scratch' && onReferenceAttachmentsChange && (
        <div className="border-t border-border pt-6">
          <ReferenceFilesUpload
            attachments={referenceAttachments}
            onAttachmentsChange={onReferenceAttachmentsChange}
          />
        </div>
      )}

      {/* Advanced Settings */}
      <div className="border-t border-border pt-6">
        <h4 className="text-sm font-bold text-card-foreground mb-4">
          Initial Settings
        </h4>
        <div className="space-y-3">
          <ToggleSetting
            label="Require Code Review"
            description="Agent changes require human approval before merging"
            checked={localConfig.customSettings.requireReview}
            onChange={(checked) => updateCustomSettings({ requireReview: checked })}
          />
          <ToggleSetting
            label="Auto-create Init Task"
            description="Create an initialization task for the agent to set up the project"
            checked={localConfig.customSettings.autoCreateInitTask}
            onChange={(checked) => updateCustomSettings({ autoCreateInitTask: checked })}
          />
          {typeInfo.supportsPreview && (
            <ToggleSetting
              label="Enable Task Preview Delivery"
              description="Publish a live preview URL or a downloadable test artifact after each completed task"
              checked={localConfig.customSettings.enablePreview}
              onChange={(checked) => updateCustomSettings({ enablePreview: checked })}
            />
          )}
        </div>
      </div>
    </div>
  );
}

interface ToggleSettingProps {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

interface TechStackOptionProps {
  stack: TechStack;
  selected: boolean;
  onSelect: (value: string) => void;
}

function TechStackOption({ stack, selected, onSelect }: TechStackOptionProps) {
  return (
    <button
      type="button"
      onClick={() => onSelect(stack.value)}
      className={`w-full px-4 py-2.5 flex items-center justify-between text-left transition-colors ${
        selected ? 'bg-primary/10' : 'hover:bg-muted'
      }`}
    >
      <div className="min-w-0">
        <p className={`text-sm font-medium ${selected ? 'text-primary' : 'text-card-foreground'}`}>
          {stack.name}
        </p>
        <p className="text-xs text-muted-foreground truncate">{stack.value}</p>
      </div>
      {selected && (
        <span className="material-symbols-outlined text-[18px] text-primary">
          check
        </span>
      )}
    </button>
  );
}

function ToggleSetting({ label, description, checked, onChange }: ToggleSettingProps) {
  return (
    <label className="flex items-start gap-3 cursor-pointer group">
      <div className="relative mt-0.5">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          className="sr-only"
        />
        <div
          className={`w-10 h-6 rounded-full transition-colors ${
            checked ? 'bg-primary' : 'bg-muted dark:bg-muted/50'
          }`}
        >
          <div
            className={`absolute top-1 w-4 h-4 rounded-full bg-white shadow transition-transform ${
              checked ? 'translate-x-5' : 'translate-x-1'
            }`}
          />
        </div>
      </div>
      <div>
        <p className="text-sm font-medium text-card-foreground group-hover:text-primary transition-colors">
          {label}
        </p>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
    </label>
  );
}

function slugifyPreview(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return '';
  const slug = trimmed
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
  return slug.slice(0, 64);
}

function getAIPlaceholder(type: ProjectType): string {
  const placeholders: Record<ProjectType, string> = {
    web: 'E.g. I need a SaaS dashboard for inventory management with real-time updates, role-based auth, and PostgreSQL backend.',
    mobile: 'E.g. A fitness tracking app with workout logging, progress charts, Apple Health integration, and offline support.',
    desktop: 'E.g. A code editor with syntax highlighting, git integration, multiple tabs, and plugin support.',
    extension: 'E.g. A browser extension that blocks distracting websites, tracks time spent, and syncs across devices.',
    api: 'E.g. A REST API for e-commerce with product catalog, shopping cart, orders, and Stripe payment integration.',
    microservice: 'E.g. A notification service handling email, SMS, and push notifications with rate limiting and retry logic.',
  };
  return placeholders[type];
}

export default StepConfigure;
