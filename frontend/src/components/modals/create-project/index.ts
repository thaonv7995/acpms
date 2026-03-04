/**
 * Create Project Modal Components
 *
 * Multi-step wizard for project creation with support for:
 * - Creating from scratch
 * - Importing from GitLab
 * - Using templates
 */

// Step components
export { StepSelectMethod, type CreationMethod } from './StepSelectMethod';
export { StepSelectType } from './StepSelectType';
export { StepConfigure, type ProjectConfig, type ConfigMode } from './StepConfigure';
export { StepReview } from './StepReview';

// Template gallery
export { TemplateGallery } from './TemplateGallery';

// Type icons
export {
  TypeIcon,
  TypeIconBadge,
  getTypeIcon,
  getTypeColors,
  projectTypeIcons,
} from './TypeIcon';

// Legacy components (kept for backward compatibility)
export { AppTypeSelect } from './AppTypeSelect';
export { GitLabImportForm } from './GitLabImportForm';
export { ManualProjectForm } from './ManualProjectForm';
export { ProjectTypeSelector } from './ProjectTypeSelector';
export { WizardFooter } from './WizardFooter';
