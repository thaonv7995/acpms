/**
 * Types for Diff Viewer components
 */

export type DiffFileStatus = 'added' | 'modified' | 'deleted' | 'renamed';
export type DiffLineType = 'add' | 'del' | 'normal';
export type ViewMode = 'side-by-side' | 'unified';

export interface DiffLine {
  type: DiffLineType;
  oldLine?: number;
  newLine?: number;
  content: string;
}

export interface DiffHunk {
  header: string;
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  changes: DiffLine[];
}

export interface DiffFile {
  path: string;
  oldPath?: string;
  status: DiffFileStatus;
  additions: number;
  deletions: number;
  isBinary: boolean;
  hunks: DiffHunk[];
  oldContent?: string;
  newContent?: string;
}

export interface BranchInfo {
  source: string;
  target: string;
  commitsAhead: number;
  commitsBehind: number;
  hasConflicts?: boolean;
}

export interface DiffSummary {
  totalFiles: number;
  totalAdditions: number;
  totalDeletions: number;
  filesAdded: number;
  filesModified: number;
  filesDeleted: number;
  filesRenamed: number;
}

export interface AvailableActions {
  canMerge: boolean;
  canCreatePR: boolean;
  canRebase: boolean;
  canReject: boolean;
}

export interface DiffResponse {
  attemptId: string;
  summary: DiffSummary;
  branchInfo: BranchInfo;
  files: DiffFile[];
  availableActions: AvailableActions;
}

// WebSocket events for real-time updates
export interface DiffUpdateEvent {
  type: 'diff_update';
  file: DiffFile;
  summary: DiffSummary;
}

export interface DiffCompleteEvent {
  type: 'diff_complete';
  branchInfo: BranchInfo;
  availableActions: AvailableActions;
}

// Status config for file status badges
export interface DiffStatusConfig {
  color: string;
  bgColor: string;
  icon: string;
  label: string;
}

export const STATUS_CONFIG: Record<DiffFileStatus, DiffStatusConfig> = {
  added: {
    color: 'text-green-500',
    bgColor: 'bg-green-500/10',
    icon: 'add_circle',
    label: 'Added',
  },
  modified: {
    color: 'text-amber-500',
    bgColor: 'bg-amber-500/10',
    icon: 'edit',
    label: 'Modified',
  },
  deleted: {
    color: 'text-red-500',
    bgColor: 'bg-red-500/10',
    icon: 'remove_circle',
    label: 'Deleted',
  },
  renamed: {
    color: 'text-blue-500',
    bgColor: 'bg-blue-500/10',
    icon: 'drive_file_rename_outline',
    label: 'Renamed',
  },
};

// Language mapping for syntax highlighting
export const LANG_MAP: Record<string, string> = {
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  py: 'python',
  rs: 'rust',
  go: 'go',
  java: 'java',
  kt: 'kotlin',
  swift: 'swift',
  rb: 'ruby',
  php: 'php',
  css: 'css',
  scss: 'scss',
  less: 'less',
  html: 'html',
  vue: 'vue',
  svelte: 'svelte',
  json: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  md: 'markdown',
  sql: 'sql',
  sh: 'bash',
  bash: 'bash',
  dockerfile: 'dockerfile',
  toml: 'toml',
  xml: 'xml',
};

export function getLanguageFromPath(filePath: string): string {
  const ext = filePath.split('.').pop()?.toLowerCase() || '';
  return LANG_MAP[ext] || 'plaintext';
}

export function parseFilePath(filePath: string): { fileName: string; dirPath: string } {
  const fileName = filePath.split('/').pop() || filePath;
  const dirPath = filePath.includes('/') ? filePath.substring(0, filePath.lastIndexOf('/')) : '';
  return { fileName, dirPath };
}
