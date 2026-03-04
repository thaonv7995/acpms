/**
 * Shared utility to resolve tech stack from project metadata, architecture config, and project type.
 * Used by both project list (ProjectCard) and project detail (SummaryTab) for consistency.
 */

const PROJECT_TYPE_DEFAULT_STACKS: Record<string, string[]> = {
  web: ['React + Vite'],
  mobile: ['React Native'],
  desktop: ['Tauri'],
  extension: ['Plasmo'],
  api: ['FastAPI'],
  microservice: ['Docker'],
};

const STACK_LABEL_OVERRIDES: Record<string, string> = {
  'react-vite': 'React + Vite',
  nextjs: 'Next.js',
  vuejs: 'Vue.js',
  nuxt3: 'Nuxt 3',
  sveltekit: 'SvelteKit',
  nestjs: 'NestJS',
  fastapi: 'FastAPI',
  'django-rest': 'Django REST',
  'spring-boot': 'Spring Boot',
  'aspnet-core': 'ASP.NET Core',
  'laravel-api': 'Laravel API',
  postgresql: 'PostgreSQL',
  mysql: 'MySQL',
  mongodb: 'MongoDB',
  redis: 'Redis',
  tauri: 'Tauri',
  electron: 'Electron',
};

function asObject(value: unknown): Record<string, unknown> {
  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value);
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      return {};
    }
  }
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function parseCompactStackString(value: string): string[] {
  return value
    .split(/[|,]/)
    .map((segment) => segment.trim())
    .filter(Boolean)
    .map((segment) => {
      const separatorIndex = segment.lastIndexOf(':');
      return separatorIndex >= 0 ? segment.slice(separatorIndex + 1).trim() : segment;
    })
    .filter(Boolean);
}

function extractStackSelections(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const stacks: string[] = [];
  for (const entry of value) {
    const obj = asObject(entry);
    const stackValue = obj.stack;
    if (typeof stackValue === 'string' && stackValue.trim().length > 0) {
      stacks.push(stackValue.trim());
    }
  }
  return stacks;
}

function extractStackValues(value: unknown): string[] {
  if (typeof value === 'string') return parseCompactStackString(value);
  if (Array.isArray(value)) {
    return value
      .filter((item): item is string => typeof item === 'string')
      .map((item) => item.trim())
      .filter(Boolean);
  }
  return [];
}

function toStackLabel(value: string): string {
  const normalized = value.trim().toLowerCase();
  if (!normalized) return '';
  if (STACK_LABEL_OVERRIDES[normalized]) return STACK_LABEL_OVERRIDES[normalized];
  return value
    .trim()
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .split(' ')
    .map((token) => (token.length === 0 ? '' : token[0].toUpperCase() + token.slice(1)))
    .join(' ')
    .trim();
}

function dedupeStackLabels(values: string[]): string[] {
  const deduped: string[] = [];
  for (const value of values) {
    if (!value) continue;
    if (deduped.some((existing) => existing.toLowerCase() === value.toLowerCase())) continue;
    deduped.push(value);
  }
  return deduped;
}

/**
 * Resolve tech stack from project DTO (metadata, architecture_config, project_type).
 * Same logic as useProjects for consistency between list and detail views.
 */
export function resolveTechStack(dto: {
  metadata?: unknown;
  architecture_config?: unknown;
  project_type?: string;
}): string[] {
  const metadata = asObject(dto.metadata);
  const architecture = asObject(dto.architecture_config);

  const rawStacks = [
    ...extractStackSelections(metadata.stackSelections),
    ...extractStackSelections(metadata.stack_selections),
    ...extractStackValues(metadata.techStack),
    ...extractStackValues(metadata.tech_stack),
    ...extractStackSelections(architecture.stackSelections),
    ...extractStackSelections(architecture.stack_selections),
    ...extractStackValues(architecture.techStack),
    ...extractStackValues(architecture.tech_stack),
  ];

  const labels = dedupeStackLabels(rawStacks.map(toStackLabel));
  if (labels.length > 0) return labels;

  if (dto.project_type) {
    return PROJECT_TYPE_DEFAULT_STACKS[dto.project_type] ?? [];
  }

  return [];
}
