import { useState } from 'react';
import { CheckCircle, XCircle, Loader2, MessageSquare } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useReview } from '@/contexts/ReviewContext';
import { logger } from '@/lib/logger';

interface ReviewActionBarProps {
  className?: string;
  onReviewSubmitted?: () => void;
}

/**
 * ReviewActionBar - Sticky action bar for approving or requesting changes
 * Shows review decision buttons and optional summary input
 *
 * @example
 * <ReviewActionBar onReviewSubmitted={handleSubmitted} />
 */
export function ReviewActionBar({
  className = '',
  onReviewSubmitted,
}: ReviewActionBarProps) {
  const {
    reviewState,
    reviewDecision,
    isSubmitting,
    setReviewDecision,
    submitReview,
  } = useReview();

  const [showSummary, setShowSummary] = useState(false);
  const [summary, setSummary] = useState('');

  const isApproved = reviewState === 'approved';
  const isChangesRequested = reviewState === 'changes_requested';
  const isPending = reviewState === 'pending';
  const isInProgress = reviewState === 'in_progress';

  const handleApprove = () => {
    setReviewDecision('approve');
    setShowSummary(true);
  };

  const handleRequestChanges = () => {
    setReviewDecision('request_changes');
    setShowSummary(true);
  };

  const handleCancel = () => {
    setReviewDecision(null);
    setShowSummary(false);
    setSummary('');
  };

  const handleSubmit = async () => {
    try {
      await submitReview(summary.trim() || undefined);
      setSummary('');
      setShowSummary(false);
      onReviewSubmitted?.();
    } catch (error) {
      logger.error('Failed to submit review:', error);
    }
  };

  return (
    <div
      className={`sticky bottom-0 bg-card border-t border-border shadow-lg ${className}`}
    >
      {/* Status Message (when review is submitted) */}
      {(isApproved || isChangesRequested) && (
        <div
          className={`px-4 py-2 text-sm font-medium ${
            isApproved
              ? 'bg-green-50 dark:bg-green-500/20 text-green-700 dark:text-green-400'
              : 'bg-orange-50 dark:bg-orange-500/20 text-orange-700 dark:text-orange-400'
          }`}
        >
          {isApproved && '✓ Review approved'}
          {isChangesRequested && '⚠ Changes requested'}
        </div>
      )}

      {/* Summary Input (when decision selected) */}
      {showSummary && !isApproved && !isChangesRequested && (
        <div className="px-4 py-3 bg-muted border-b border-border">
          <label className="block text-sm font-medium text-card-foreground mb-2">
            Review Summary {reviewDecision === 'request_changes' && '(Required)'}
          </label>
          <textarea
            value={summary}
            onChange={(e) => setSummary(e.target.value)}
            placeholder={
              reviewDecision === 'approve'
                ? 'Optional summary of your approval...'
                : 'Explain what changes are needed...'
            }
            className="w-full px-3 py-2 text-sm border border-border rounded bg-card text-card-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50"
            rows={3}
            maxLength={2000}
          />
          <div className="mt-1 text-xs text-muted-foreground">
            {summary.length}/2000 characters
          </div>
        </div>
      )}

      {/* Action Buttons */}
      <div className="px-4 py-3 flex items-center justify-between gap-3">
        {/* Left: Review State Info */}
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <MessageSquare className="w-4 h-4" />
          <span>
            {isPending && 'No review started'}
            {isInProgress && 'Review in progress'}
            {isApproved && 'Approved'}
            {isChangesRequested && 'Changes requested'}
          </span>
        </div>

        {/* Right: Decision Buttons */}
        {!isApproved && !isChangesRequested && (
          <div className="flex items-center gap-2">
            {showSummary ? (
              // Summary mode: Show Cancel + Submit
              <>
                <Button onClick={handleCancel} variant="ghost" size="sm">
                  Cancel
                </Button>
                <Button
                  onClick={handleSubmit}
                  disabled={
                    isSubmitting ||
                    (reviewDecision === 'request_changes' && !summary.trim())
                  }
                  variant="default"
                  size="sm"
                  className="gap-2"
                >
                  {isSubmitting ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" />
                      Submitting...
                    </>
                  ) : (
                    <>
                      {reviewDecision === 'approve' ? (
                        <>
                          <CheckCircle className="w-4 h-4" />
                          Submit Approval
                        </>
                      ) : (
                        <>
                          <XCircle className="w-4 h-4" />
                          Submit Feedback
                        </>
                      )}
                    </>
                  )}
                </Button>
              </>
            ) : (
              // Decision mode: Show Approve + Request Changes
              <>
                <Button
                  onClick={handleApprove}
                  variant="outline"
                  size="sm"
                  className="gap-2 border-green-600 text-green-700 hover:bg-green-50 dark:border-green-500 dark:text-green-400 dark:hover:bg-green-500/20"
                >
                  <CheckCircle className="w-4 h-4" />
                  Approve
                </Button>
                <Button
                  onClick={handleRequestChanges}
                  variant="outline"
                  size="sm"
                  className="gap-2 border-orange-600 text-orange-700 hover:bg-orange-50 dark:border-orange-500 dark:text-orange-400 dark:hover:bg-orange-500/20"
                >
                  <XCircle className="w-4 h-4" />
                  Request Changes
                </Button>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
