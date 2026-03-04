/**
 * TemplateGallery - Browse and select project templates
 *
 * Features:
 * - Filter by project type
 * - Search templates
 * - Preview template details
 * - Select template for project creation
 */

import { useState, useMemo } from 'react';
import { useTemplates } from '../../../hooks/useTemplates';
import {
  type ProjectType,
  type ProjectTemplate,
  getAllProjectTypes,
} from '../../../api/templates';
import { TypeIconBadge } from './TypeIcon';

interface TemplateGalleryProps {
  onSelectTemplate: (template: ProjectTemplate) => void;
  onBack: () => void;
}

export function TemplateGallery({ onSelectTemplate, onBack }: TemplateGalleryProps) {
  const [selectedType, setSelectedType] = useState<ProjectType | 'all'>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedTemplate, setSelectedTemplate] = useState<ProjectTemplate | null>(null);

  const { templates, isLoading, error } = useTemplates({
    projectType: selectedType === 'all' ? undefined : selectedType,
  });

  const projectTypes = getAllProjectTypes();

  // Filter templates by search query
  const filteredTemplates = useMemo(() => {
    if (!searchQuery.trim()) return templates;
    const query = searchQuery.toLowerCase();
    return templates.filter(
      (t) =>
        t.name.toLowerCase().includes(query) ||
        t.description?.toLowerCase().includes(query)
    );
  }, [templates, searchQuery]);

  const handleSelect = () => {
    if (selectedTemplate) {
      onSelectTemplate(selectedTemplate);
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="mb-4">
        <p className="text-sm text-muted-foreground">
          Choose a template to kickstart your project with pre-configured settings and best practices.
        </p>
      </div>

      {/* Filters */}
      <div className="flex flex-col sm:flex-row gap-3 mb-4">
        {/* Search */}
        <div className="relative flex-1">
          <span className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground material-symbols-outlined text-[20px]">
            search
          </span>
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search templates..."
            className="w-full bg-muted border border-border rounded-lg py-2 pl-10 pr-4 text-sm text-card-foreground focus:ring-primary focus:border-primary"
          />
        </div>

        {/* Type filter */}
        <select
          value={selectedType}
          onChange={(e) => setSelectedType(e.target.value as ProjectType | 'all')}
          className="bg-muted border border-border rounded-lg py-2 px-3 text-sm text-card-foreground focus:ring-primary focus:border-primary"
        >
          <option value="all">All Types</option>
          {projectTypes.map((type) => (
            <option key={type.type} value={type.type}>
              {type.label}
            </option>
          ))}
        </select>
      </div>

      {/* Template Grid */}
      <div className="flex-1 overflow-y-auto min-h-0">
        {isLoading ? (
          <LoadingState />
        ) : error ? (
          <ErrorState error={error} />
        ) : filteredTemplates.length === 0 ? (
          <EmptyState searchQuery={searchQuery} />
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            {filteredTemplates.map((template) => (
              <TemplateCard
                key={template.id}
                template={template}
                isSelected={selectedTemplate?.id === template.id}
                onSelect={() => setSelectedTemplate(template)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="mt-4 pt-4 border-t border-border flex justify-between items-center">
        <button
          onClick={onBack}
          className="text-sm font-bold text-muted-foreground hover:text-card-foreground transition-colors flex items-center gap-1"
        >
          <span className="material-symbols-outlined text-[18px]">arrow_back</span>
          Back
        </button>
        <button
          onClick={handleSelect}
          disabled={!selectedTemplate}
          className="px-6 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Use Template
          <span className="material-symbols-outlined text-[18px]">arrow_forward</span>
        </button>
      </div>
    </div>
  );
}

interface TemplateCardProps {
  template: ProjectTemplate;
  isSelected: boolean;
  onSelect: () => void;
}

function TemplateCard({ template, isSelected, onSelect }: TemplateCardProps) {
  return (
    <button
      onClick={onSelect}
      className={`relative flex flex-col p-4 rounded-xl bg-card border-2 transition-all text-left ${
        isSelected
          ? 'border-primary ring-2 ring-primary/20'
          : 'border-border hover:border-border/80'
      }`}
    >
      {/* Selection indicator */}
      {isSelected && (
        <div className="absolute top-3 right-3">
          <span className="material-symbols-outlined text-primary text-xl">check_circle</span>
        </div>
      )}

      {/* Official badge */}
      {template.is_official && (
        <div className="absolute top-3 left-3">
          <span className="px-2 py-0.5 text-[10px] font-bold rounded bg-emerald-100 dark:bg-emerald-500/20 text-emerald-700 dark:text-emerald-400">
            Official
          </span>
        </div>
      )}

      <div className="flex items-start gap-3 mt-4">
        <TypeIconBadge type={template.project_type} size="sm" />
        <div className="flex-1 min-w-0">
          <h4 className="font-bold text-card-foreground text-sm truncate">
            {template.name}
          </h4>
          <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
            {template.description || 'No description'}
          </p>
        </div>
      </div>

      {/* Tech stack tags */}
      {template.tech_stack.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-3">
          {template.tech_stack.slice(0, 3).map((tech) => (
            <span
              key={tech.value}
              className="px-2 py-0.5 text-[10px] rounded bg-muted text-card-foreground"
            >
              {tech.name}
            </span>
          ))}
          {template.tech_stack.length > 3 && (
            <span className="px-2 py-0.5 text-[10px] rounded bg-muted text-muted-foreground">
              +{template.tech_stack.length - 3}
            </span>
          )}
        </div>
      )}
    </button>
  );
}

function LoadingState() {
  return (
    <div className="flex flex-col items-center justify-center py-12">
      <div className="size-8 border-2 border-primary border-t-transparent rounded-full animate-spin mb-3" />
      <p className="text-sm text-muted-foreground">Loading templates...</p>
    </div>
  );
}

function ErrorState({ error }: { error: Error }) {
  return (
    <div className="flex flex-col items-center justify-center py-12">
      <span className="material-symbols-outlined text-red-500 text-4xl mb-3">error</span>
      <p className="text-sm text-card-foreground font-medium mb-1">
        Failed to load templates
      </p>
      <p className="text-xs text-muted-foreground">{error.message}</p>
    </div>
  );
}

function EmptyState({ searchQuery }: { searchQuery: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-12">
      <span className="material-symbols-outlined text-muted-foreground text-4xl mb-3">
        {searchQuery ? 'search_off' : 'folder_open'}
      </span>
      <p className="text-sm text-card-foreground font-medium mb-1">
        {searchQuery ? 'No templates found' : 'No templates available'}
      </p>
      <p className="text-xs text-muted-foreground">
        {searchQuery
          ? 'Try adjusting your search or filter'
          : 'Templates will appear here when added'}
      </p>
    </div>
  );
}

export default TemplateGallery;
