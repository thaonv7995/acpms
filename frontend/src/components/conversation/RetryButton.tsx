import { RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useRetryUi } from '@/contexts/RetryUiContext';

export interface RetryButtonProps {
  processId: string;
}

export function RetryButton({ processId }: RetryButtonProps) {
  const { setRetryProcessId, isRetrying } = useRetryUi();

  const handleRetry = () => {
    setRetryProcessId(processId);
  };

  return (
    <Button
      onClick={handleRetry}
      disabled={isRetrying}
      variant="outline"
      size="sm"
      className="gap-1.5"
    >
      <RotateCcw className="h-4 w-4" />
      <span>Retry</span>
    </Button>
  );
}
