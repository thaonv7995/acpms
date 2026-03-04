/**
 * StepSelectType - Step 2: Select Project Type
 *
 * Displays 6 project type cards:
 * - Web Application
 * - Mobile App
 * - Desktop App
 * - Browser Extension
 * - API Service
 * - Microservice
 */

import { TypeIconBadge } from './TypeIcon';
import { type ProjectType, getAllProjectTypes, type ProjectTypeInfo } from '../../../api/templates';

interface StepSelectTypeProps {
  selectedType: ProjectType | null;
  onSelectType: (type: ProjectType) => void;
}

export function StepSelectType({ selectedType, onSelectType }: StepSelectTypeProps) {
  const projectTypes = getAllProjectTypes();

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground mb-6">
        Select the type of project you're building. This helps us configure the right defaults and tooling.
      </p>

      <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
        {projectTypes.map((typeInfo) => (
          <ProjectTypeCard
            key={typeInfo.type}
            typeInfo={typeInfo}
            isSelected={selectedType === typeInfo.type}
            onSelect={() => onSelectType(typeInfo.type)}
          />
        ))}
      </div>

      {/* Selected type details */}
      {selectedType && (
        <SelectedTypeDetails
          typeInfo={projectTypes.find((t) => t.type === selectedType)!}
        />
      )}
    </div>
  );
}

interface ProjectTypeCardProps {
  typeInfo: ProjectTypeInfo;
  isSelected: boolean;
  onSelect: () => void;
}

function ProjectTypeCard({ typeInfo, isSelected, onSelect }: ProjectTypeCardProps) {
  return (
    <button
      onClick={onSelect}
      className={`group relative flex flex-col items-start p-4 rounded-xl bg-card border-2 transition-all text-left ${
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

      <TypeIconBadge type={typeInfo.type} size="md" className="mb-3" />

      <h4 className="font-bold text-card-foreground text-sm mb-1">
        {typeInfo.label}
      </h4>
      <p className="text-xs text-muted-foreground leading-relaxed line-clamp-2">
        {typeInfo.description}
      </p>

      {/* Preview badge */}
      {typeInfo.supportsPreview && (
        <div className="mt-3 flex items-center gap-1">
          <span className="material-symbols-outlined text-emerald-500 text-[14px]">visibility</span>
          <span className="text-[10px] text-emerald-500 font-medium">Task Preview</span>
        </div>
      )}
    </button>
  );
}

interface SelectedTypeDetailsProps {
  typeInfo: ProjectTypeInfo;
}

function SelectedTypeDetails({ typeInfo }: SelectedTypeDetailsProps) {
  return (
    <div className="mt-6 p-4 rounded-lg bg-muted border border-border">
      <div className="flex items-start gap-4">
        <TypeIconBadge type={typeInfo.type} size="lg" />
        <div className="flex-1">
          <h4 className="text-sm font-bold text-card-foreground mb-1">
            {typeInfo.label} Selected
          </h4>
          <p className="text-xs text-muted-foreground mb-3">
            {typeInfo.description}
          </p>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <p className="text-[10px] uppercase font-bold text-muted-foreground mb-1">
                Popular Tech Stacks
              </p>
              <div className="flex flex-wrap gap-1">
                {typeInfo.defaultTechStacks.slice(0, 3).map((stack) => (
                  <span
                    key={stack.value}
                    className="px-2 py-0.5 text-[10px] rounded bg-muted dark:bg-muted/50 text-card-foreground"
                  >
                    {stack.name}
                  </span>
                ))}
              </div>
            </div>
            <div>
              <p className="text-[10px] uppercase font-bold text-muted-foreground mb-1">
                Default Build
              </p>
              <code className="text-[10px] text-muted-foreground font-mono">
                {typeInfo.defaultBuildCommand}
              </code>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default StepSelectType;
