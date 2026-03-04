import { ArrowRight, AlertCircle, Settings, CheckCircle } from 'lucide-react';
import { BaseEntry } from './BaseEntry';

interface NextActionCardProps {
  failed: boolean;
  executionProcesses: number;
  needsSetup: boolean;
  timestamp?: string | null;
}

/**
 * Display next action information for agent execution.
 */
export function NextActionCard({
  failed,
  executionProcesses,
  needsSetup,
  timestamp,
}: NextActionCardProps) {
  const variant = failed ? 'error' : 'action';
  const Icon = failed ? AlertCircle : needsSetup ? Settings : CheckCircle;
  const iconColor = failed
    ? 'text-destructive'
    : needsSetup
    ? 'text-orange-500'
    : 'text-green-500';

  return (
    <BaseEntry variant={variant} timestamp={timestamp}>
      <div className="flex items-start gap-3">
        <Icon className={`w-5 h-5 ${iconColor} flex-shrink-0 mt-0.5`} />
        <div className="flex-1 min-w-0">
          <div className="font-medium text-sm mb-1">
            {failed
              ? 'Action Failed'
              : needsSetup
              ? 'Setup Required'
              : 'Next Action'}
          </div>
          <div className="text-xs text-muted-foreground flex items-center gap-2">
            <ArrowRight className="w-3 h-3" />
            <span>
              {executionProcesses} execution process
              {executionProcesses !== 1 ? 'es' : ''}
              {needsSetup && ' (setup pending)'}
            </span>
          </div>
        </div>
      </div>
    </BaseEntry>
  );
}
