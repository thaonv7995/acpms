// Task Metadata Types
// These types define metadata structures for different task types

/**
 * Source of project initialization
 */
export type InitSource = 'gitlab_import' | 'from_scratch';

/**
 * Metadata for init tasks
 */
export interface InitTaskMetadata {
  source: InitSource;
  repository_url?: string;  // For gitlab_import
  visibility?: 'private' | 'public' | 'internal';  // For from_scratch
}

/**
 * Union type for all task metadata
 */
export interface TaskMetadata {
  init?: InitTaskMetadata;
  // Add other task type metadata here as needed
}
