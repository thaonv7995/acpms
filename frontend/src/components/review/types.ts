// Review Comment Types for Code Review UI

/**
 * Comment type categorization
 * - general: Overall review comment not tied to specific file/line
 * - file: Comment about a specific file
 * - line: Comment tied to a specific line in the diff
 */
export type CommentType = 'general' | 'file' | 'line';

/**
 * Review comment structure matching backend schema
 */
export interface ReviewComment {
  id: string;
  attempt_id: string;
  user_id: string;
  user_name: string;
  user_avatar?: string;
  file_path?: string | null;    // null for general comment
  line_number?: number | null;  // null for file-level comment
  content: string;
  resolved: boolean;
  resolved_by?: string | null;
  resolved_by_name?: string | null;
  created_at: string;
  updated_at: string;
}

/**
 * Request payload for adding a new comment
 */
export interface AddCommentRequest {
  attempt_id: string;
  content: string;
  file_path?: string;
  line_number?: number;
}

/**
 * Request payload for rejecting an attempt
 */
export interface RejectRequest {
  reason: string;
}

/**
 * Request payload for requesting changes
 */
export interface RequestChangesRequest {
  feedback: string;
  include_comments?: boolean;
}

/**
 * Request payload for approving an attempt
 */
export interface ApproveRequest {
  commit_message?: string;
}

/**
 * Review action type for button states
 */
export type ReviewAction = 'approve' | 'reject' | 'request-changes';

/**
 * Props for review action handlers
 */
export interface ReviewActionHandlers {
  onApprove: (commitMessage?: string) => Promise<void>;
  onReject: (reason: string) => Promise<void>;
  onRequestChanges: (request: RequestChangesRequest) => Promise<void>;
}

/**
 * Comments grouped by file path for display
 */
export interface CommentsByFile {
  filePath: string | null; // null for general comments
  comments: ReviewComment[];
}

/**
 * Line comment position for popover placement
 */
export interface LineCommentPosition {
  filePath: string;
  lineNumber: number;
  lineType: 'add' | 'delete' | 'normal';
  x: number;
  y: number;
}

/**
 * Review status derived from attempt and comments
 */
export interface ReviewStatus {
  hasUnresolvedComments: boolean;
  totalComments: number;
  resolvedComments: number;
  canApprove: boolean;
}
