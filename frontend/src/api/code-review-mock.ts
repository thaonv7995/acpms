import { Comment } from '@/components/code-review/InlineComment';
import { DiffComments, ReviewDecision } from '@/contexts/ReviewContext';
import { logger } from '@/lib/logger';

/**
 * Mock API for code review operations
 * This simulates backend responses for Phase 5.6 (frontend-only)
 */

/**
 * Create new comment object
 */
export function createComment(
  currentUserId: string,
  text: string,
  lineNumber: number
): Comment {
  return {
    id: `comment-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
    authorId: currentUserId,
    author: {
      firstName: 'Current',
      lastName: 'User',
      imageUrl: undefined,
    },
    text,
    createdAt: new Date().toISOString(),
    lineNumber,
  };
}

/**
 * Remove comment from comments structure
 */
export function removeComment(
  comments: DiffComments,
  fileId: string,
  lineNumber: number,
  commentId: string
): DiffComments {
  const fileComments = comments[fileId];
  if (!fileComments) return comments;

  const lineComments = fileComments[lineNumber];
  if (!lineComments) return comments;

  const updatedLineComments = lineComments.filter((c) => c.id !== commentId);

  const updatedFileComments = { ...fileComments };
  if (updatedLineComments.length === 0) {
    delete updatedFileComments[lineNumber];
  } else {
    updatedFileComments[lineNumber] = updatedLineComments;
  }

  if (Object.keys(updatedFileComments).length === 0) {
    const { [fileId]: _, ...rest } = comments;
    return rest;
  }

  return {
    ...comments,
    [fileId]: updatedFileComments,
  };
}

/**
 * Count total comments
 */
export function countComments(comments: DiffComments): number {
  return Object.values(comments).reduce(
    (total, fileComments) =>
      total +
      Object.values(fileComments).reduce(
        (fileTotal, lineComments) => fileTotal + lineComments.length,
        0
      ),
    0
  );
}

/**
 * Mock: Submit review
 * Simulates 1s API delay
 */
export async function submitReviewMock(
  attemptId: string,
  decision: ReviewDecision,
  comments: DiffComments,
  summary?: string
): Promise<void> {
  logger.log('[MOCK] Submitting review:', {
    attemptId,
    decision,
    summary,
    commentCount: countComments(comments),
  });

  // Simulate API delay
  await new Promise((resolve) => setTimeout(resolve, 1000));

  // TODO: Replace with real API call
  // await axios.post(`/api/v1/attempts/${attemptId}/reviews/submit`, {
  //   decision,
  //   comments,
  //   summary,
  // });
}

/**
 * Future: Real API implementations
 */

// export async function addCommentApi(
//   attemptId: string,
//   fileId: string,
//   lineNumber: number,
//   text: string
// ): Promise<Comment> {
//   const response = await axios.post(`/api/v1/attempts/${attemptId}/reviews/comments`, {
//     fileId,
//     lineNumber,
//     text,
//   });
//   return response.data;
// }

// export async function deleteCommentApi(commentId: string): Promise<void> {
//   await axios.delete(`/api/v1/reviews/comments/${commentId}`);
// }

// export async function getReviewCommentsApi(attemptId: string): Promise<DiffComments> {
//   const response = await axios.get(`/api/v1/attempts/${attemptId}/reviews/comments`);
//   return response.data;
// }
