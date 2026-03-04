// Diff Types for git-diff-view integration

export type DiffStatus = 'added' | 'modified' | 'deleted' | 'renamed';

export interface FileDiff {
  id: string;
  attempt_id: string;
  file_path: string;
  old_path?: string; // For renamed files
  status: DiffStatus;
  additions: number;
  deletions: number;
  hunks: DiffHunk[];
  old_content?: string;
  new_content?: string;
  is_binary: boolean;
  created_at: string;
}

export interface DiffHunk {
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  content: string;
  changes: DiffChange[];
}

export interface DiffChange {
  type: 'add' | 'delete' | 'normal';
  content: string;
  old_line_number?: number;
  new_line_number?: number;
}

export interface DiffSummary {
  total_files: number;
  total_additions: number;
  total_deletions: number;
  files_added: number;
  files_modified: number;
  files_deleted: number;
  files_renamed: number;
}

// WebSocket streaming types
export interface DiffStreamMessage {
  type: 'diff_update' | 'diff_complete' | 'diff_error';
  attempt_id: string;
  payload: FileDiff | DiffSummary | { error: string };
}

export interface DiffPatch {
  op: 'add' | 'remove' | 'replace';
  path: string;
  value?: unknown;
}

// API response types
export interface DiffResponse {
  diffs: FileDiff[];
  summary: DiffSummary;
  branch_status?: BranchStatus;
}

export interface BranchStatus {
  branch_name: string;
  target_branch_name: string;
  ahead_count: number;
  behind_count: number;
  has_conflicts: boolean;
  can_push: boolean;
  can_merge: boolean;
  pr_url?: string;
  pr_status?: 'open' | 'merged' | 'closed';
}
