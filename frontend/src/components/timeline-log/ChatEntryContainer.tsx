/**
 * Phase 3: Vibe Kanban-style entry container.
 * Header (icon, title, actions) + expandable content + optional actions footer.
 */
import type { ComponentType } from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';

export type ChatEntryVariant =
  | 'user'
  | 'assistant'
  | 'tool'
  | 'thinking'
  | 'system'
  | 'error'
  | 'plan'
  | 'plan_denied';

interface VariantConfig {
  headerBg: string;
  border: string;
  bg: string;
}

const variantConfig: Record<ChatEntryVariant, VariantConfig> = {
  user: {
    headerBg: 'bg-muted/20',
    border: 'border-border',
    bg: 'bg-muted/5',
  },
  assistant: {
    headerBg: 'bg-primary/5',
    border: 'border-primary/20',
    bg: 'bg-transparent',
  },
  tool: {
    headerBg: 'bg-muted/30',
    border: 'border-border',
    bg: 'bg-muted/10',
  },
  thinking: {
    headerBg: 'bg-muted/20',
    border: 'border-border',
    bg: 'bg-muted/5',
  },
  system: {
    headerBg: 'bg-muted/30',
    border: 'border-border',
    bg: 'bg-muted/10',
  },
  error: {
    headerBg: 'bg-destructive/10',
    border: 'border-destructive/30',
    bg: 'bg-destructive/5',
  },
  plan: {
    headerBg: 'bg-primary/20',
    border: 'border-primary',
    bg: 'bg-primary/10',
  },
  plan_denied: {
    headerBg: 'bg-destructive/20',
    border: 'border-destructive',
    bg: 'bg-destructive/10',
  },
};

export interface ChatEntryContainerProps {
  variant: ChatEntryVariant;
  icon?: ComponentType<{ className?: string }>;
  title?: React.ReactNode;
  headerRight?: React.ReactNode;
  /** Optional badge shown in header (e.g. "Queued" for user messages) */
  badge?: React.ReactNode;
  expanded?: boolean;
  onToggle?: () => void;
  children?: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
  /** When true, content is always visible (no expand/collapse) */
  alwaysExpanded?: boolean;
}

export function ChatEntryContainer({
  variant,
  icon: Icon,
  title,
  headerRight,
  badge,
  expanded = false,
  onToggle,
  children,
  actions,
  className,
  alwaysExpanded = false,
}: ChatEntryContainerProps) {
  const config = variantConfig[variant];
  const hasToggle = Boolean(onToggle) && !alwaysExpanded;

  return (
    <div
      className={cn(
        'rounded-lg border w-full overflow-hidden',
        config.border,
        config.bg,
        className
      )}
    >
      {/* Header row */}
      <div
        className={cn(
          'flex items-center gap-2 px-3 py-2 min-h-[36px]',
          config.headerBg,
          hasToggle && 'cursor-pointer hover:opacity-90 transition-opacity'
        )}
        onClick={hasToggle ? onToggle : undefined}
        role={hasToggle ? 'button' : undefined}
        tabIndex={hasToggle ? 0 : undefined}
        onKeyDown={
          hasToggle
            ? (e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  onToggle?.();
                }
              }
            : undefined
        }
      >
        {Icon && <Icon className="h-4 w-4 shrink-0 text-muted-foreground" />}
        {title != null && (
          <span className="flex-1 text-sm font-medium text-foreground truncate min-w-0">
            {title}
          </span>
        )}
        {badge}
        {headerRight}
        {hasToggle && (
          <span className="shrink-0 text-muted-foreground">
            <ChevronDown
              className={cn('h-4 w-4 transition-transform', !expanded && '-rotate-90')}
            />
          </span>
        )}
      </div>

      {/* Content - when expanded or alwaysExpanded */}
      {(alwaysExpanded || expanded) && children && (
        <div className="px-3 py-2 border-t border-border/40 text-sm">{children}</div>
      )}

      {/* Actions footer */}
      {actions && (
        <div className="flex items-center gap-2 px-3 py-2 border-t border-border/40 bg-primary/20 backdrop-blur-sm">
          {actions}
        </div>
      )}
    </div>
  );
}
