// Task Attempt Types for vibe-kanban pattern

export type AttemptStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface TaskAttempt {
  id: string;
  task_id: string;
  executor?: string;
  variant?: string;
  branch?: string;
  metadata?: Record<string, unknown>;
  status: AttemptStatus;
  started_at?: string;
  completed_at?: string;
  ended_at?: string;
  error_message?: string;
  logs_count?: number;
  diffs_count?: number;
  created_at: string;
  updated_at?: string;
}

export interface AttemptLog {
  id: string;
  attempt_id: string;
  timestamp: string;
  level: 'debug' | 'info' | 'warn' | 'error';
  message: string;
  metadata?: Record<string, unknown>;
}

export interface AttemptDiff {
  id: string;
  attempt_id: string;
  file_path: string;
  old_content?: string;
  new_content?: string;
  diff_type: 'added' | 'modified' | 'deleted' | 'renamed';
  additions: number;
  deletions: number;
  created_at: string;
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

export interface CreateAttemptRequest {
  task_id: string;
  executor: string;
  variant?: string;
  base_branch: string;
  prompt?: string;
}
