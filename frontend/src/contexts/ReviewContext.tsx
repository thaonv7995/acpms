import { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import { Comment } from '@/components/code-review/InlineComment';
import { createComment, removeComment, submitReviewMock } from '@/api/code-review-mock';
import { logger } from '@/lib/logger';

export type ReviewState = 'pending' | 'in_progress' | 'approved' | 'changes_requested';
export type ReviewDecision = 'approve' | 'request_changes' | null;

export interface DiffComments {
  [fileId: string]: {
    [lineNumber: number]: Comment[];
  };
}

interface ReviewContextValue {
  reviewState: ReviewState;
  comments: DiffComments;
  reviewDecision: ReviewDecision;
  isSubmitting: boolean;
  addComment: (fileId: string, lineNumber: number, text: string) => void;
  deleteComment: (fileId: string, lineNumber: number, commentId: string) => void;
  setReviewDecision: (decision: ReviewDecision) => void;
  submitReview: (summary?: string) => Promise<void>;
  resetReview: () => void;
}

const ReviewContext = createContext<ReviewContextValue | undefined>(undefined);

interface ReviewProviderProps {
  children: ReactNode;
  attemptId: string;
  currentUserId: string;
}

/**
 * ReviewProvider - Manages code review state and actions
 */
export function ReviewProvider({ children, attemptId, currentUserId }: ReviewProviderProps) {
  const [reviewState, setReviewState] = useState<ReviewState>('pending');
  const [comments, setComments] = useState<DiffComments>({});
  const [reviewDecision, setReviewDecisionState] = useState<ReviewDecision>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const addComment = useCallback(
    (fileId: string, lineNumber: number, text: string) => {
      const newComment = createComment(currentUserId, text, lineNumber);

      setComments((prev) => {
        const fileComments = prev[fileId] || {};
        const lineComments = fileComments[lineNumber] || [];

        return {
          ...prev,
          [fileId]: {
            ...fileComments,
            [lineNumber]: [...lineComments, newComment],
          },
        };
      });

      if (reviewState === 'pending') {
        setReviewState('in_progress');
      }
    },
    [currentUserId, reviewState]
  );

  const deleteComment = useCallback(
    (fileId: string, lineNumber: number, commentId: string) => {
      setComments((prev) => removeComment(prev, fileId, lineNumber, commentId));
    },
    []
  );

  const setReviewDecision = useCallback((decision: ReviewDecision) => {
    setReviewDecisionState(decision);
  }, []);

  const submitReview = useCallback(
    async (summary?: string) => {
      if (!reviewDecision) return;

      setIsSubmitting(true);
      try {
        await submitReviewMock(attemptId, reviewDecision, comments, summary);
        const newState: ReviewState =
          reviewDecision === 'approve' ? 'approved' : 'changes_requested';
        setReviewState(newState);
      } catch (error) {
        logger.error('[ReviewContext] Failed to submit review:', error);
        throw error;
      } finally {
        setIsSubmitting(false);
      }
    },
    [reviewDecision, comments, attemptId]
  );

  const resetReview = useCallback(() => {
    setReviewState('pending');
    setComments({});
    setReviewDecisionState(null);
    setIsSubmitting(false);
  }, []);

  return (
    <ReviewContext.Provider
      value={{
        reviewState,
        comments,
        reviewDecision,
        isSubmitting,
        addComment,
        deleteComment,
        setReviewDecision,
        submitReview,
        resetReview,
      }}
    >
      {children}
    </ReviewContext.Provider>
  );
}

export function useReview() {
  const context = useContext(ReviewContext);
  if (!context) {
    throw new Error('useReview must be used within ReviewProvider');
  }
  return context;
}
