import { Download, Eye, FileDiff, X, MoreHorizontal } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { LayoutMode } from '../layout/TasksLayout';
import type { KanbanTask } from '../../types/project';
import type { TaskAttempt } from '../../types/task-attempt';
import type { ArtifactDownloadRef } from '@/lib/artifact-downloads';

export interface AttemptArtifactDownloadItem extends ArtifactDownloadRef {}

interface AttemptHeaderActionsProps {
  onClose: () => void;
  mode?: LayoutMode;
  onModeChange?: (mode: LayoutMode) => void;
  task: KanbanTask;
  attempt?: TaskAttempt | null;
  showPreviewToggle?: boolean;
  previewDisabled?: boolean;
  previewDisabledReason?: string;
  artifactDownloads?: AttemptArtifactDownloadItem[];
  downloadDisabled?: boolean;
  downloadDisabledReason?: string;
  onDownloadArtifact?: (artifact?: AttemptArtifactDownloadItem) => void;
  onCreateAttempt?: () => void;
  onOpenGitActions?: () => void;
  onDeleteTask?: () => void;
}

export function AttemptHeaderActions({
  onClose,
  mode,
  onModeChange,
  task,
  attempt,
  showPreviewToggle = true,
  previewDisabled = false,
  previewDisabledReason,
  artifactDownloads = [],
  downloadDisabled = false,
  downloadDisabledReason,
  onDownloadArtifact,
  onCreateAttempt,
  onOpenGitActions,
  onDeleteTask,
}: AttemptHeaderActionsProps) {
  const enabledDownloads = artifactDownloads.filter(
    (artifact) => Boolean(artifact.legacyUrl || (artifact.attemptId && artifact.artifactId))
  );
  const hasMenuItems = Boolean(
    (task && onCreateAttempt) ||
      (attempt && onOpenGitActions) ||
      (task && onDeleteTask)
  );
  const hasDownloadAction = Boolean(
    enabledDownloads.length > 0 || downloadDisabledReason || downloadDisabled
  );
  const toggleValue = hasDownloadAction
    ? mode === 'diffs'
      ? 'diffs'
      : ''
    : mode ?? '';

  const handleDownloadArtifact = (artifact?: AttemptArtifactDownloadItem) => {
    if (downloadDisabled) return;
    if (onDownloadArtifact) {
      onDownloadArtifact(artifact);
      return;
    }
    const fallbackUrl = artifact?.legacyUrl ?? enabledDownloads[0]?.legacyUrl;
    if (fallbackUrl) {
      window.open(fallbackUrl, '_blank', 'noopener,noreferrer');
    }
  };

  return (
    <>
      {typeof mode !== 'undefined' && onModeChange && (
        <div className="inline-flex items-center gap-1">
          {hasDownloadAction ? (
            enabledDownloads.length > 1 && !downloadDisabled ? (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-8 w-8 p-0"
                    title="Download artifact"
                  >
                    <Download className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuLabel className="text-xs font-semibold uppercase tracking-wide">
                    Download artifact
                  </DropdownMenuLabel>
                  {enabledDownloads.map((artifact, index) => (
                    <DropdownMenuItem
                      key={`${artifact.artifactId ?? artifact.legacyUrl ?? artifact.label}-${index}`}
                      onClick={() => handleDownloadArtifact(artifact)}
                    >
                      {artifact.label}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                className="h-8 w-8 p-0"
                onClick={() => handleDownloadArtifact(enabledDownloads[0])}
                disabled={downloadDisabled || enabledDownloads.length === 0}
                title={
                  downloadDisabledReason ||
                  enabledDownloads[0]?.label ||
                  'Download artifact'
                }
              >
                <Download className="h-4 w-4" />
              </Button>
            )
          ) : (
            <ToggleGroup
              type="single"
              value={toggleValue}
              onValueChange={(v) => {
                const newMode = (v as LayoutMode) || null;
                onModeChange(newMode);
              }}
              className="inline-flex gap-1"
            >
              {showPreviewToggle && (
                <ToggleGroupItem
                  value="preview"
                  aria-label="Preview"
                  active={mode === 'preview'}
                  disabled={previewDisabled}
                  title={previewDisabledReason}
                >
                  <Eye className="h-4 w-4" />
                </ToggleGroupItem>
              )}
              <ToggleGroupItem
                value="diffs"
                aria-label="Diffs"
                active={mode === 'diffs'}
              >
                <FileDiff className="h-4 w-4" />
              </ToggleGroupItem>
            </ToggleGroup>
          )}

          {hasDownloadAction && (
            <ToggleGroup
              type="single"
              value={toggleValue}
              onValueChange={(v) => {
                const newMode = (v as LayoutMode) || null;
                onModeChange(newMode);
              }}
              className="inline-flex gap-1"
            >
              <ToggleGroupItem
                value="diffs"
                aria-label="Diffs"
                active={mode === 'diffs'}
              >
                <FileDiff className="h-4 w-4" />
              </ToggleGroupItem>
            </ToggleGroup>
          )}
        </div>
      )}
      {typeof mode !== 'undefined' && onModeChange && (
        <div className="h-4 w-px bg-border" />
      )}
      {hasMenuItems && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="sm" className="h-8 w-8 p-0">
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            {task && onCreateAttempt && (
              <DropdownMenuItem onClick={onCreateAttempt}>
                Create New Attempt
              </DropdownMenuItem>
            )}
            {attempt && onOpenGitActions && (
              <DropdownMenuItem onClick={onOpenGitActions}>
                Git Actions
              </DropdownMenuItem>
            )}
            {task && onDeleteTask && (
              <DropdownMenuItem
                className="text-destructive"
                onClick={onDeleteTask}
              >
                Delete Task
              </DropdownMenuItem>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
      <Button variant="ghost" size="sm" onClick={onClose} className="h-8 w-8 p-0">
        <X className="h-4 w-4" />
      </Button>
    </>
  );
}
