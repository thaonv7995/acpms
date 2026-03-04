import { useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { CreateAttemptRequest } from '@/types/task-attempt';
import { logger } from '@/lib/logger';

interface Task {
  id: string;
  title: string;
  description?: string;
}

interface NewAttemptDialogProps {
  task: Task;
  open: boolean;
  onClose: () => void;
  onSubmit: (data: CreateAttemptRequest) => Promise<void>;
  isLoading?: boolean;
}

const EXECUTOR_OPTIONS = [
  { value: 'claude-sonnet-4', label: 'Claude Sonnet 4' },
  { value: 'claude-opus-4', label: 'Claude Opus 4' },
  { value: 'claude-haiku-4', label: 'Claude Haiku 4' },
];

const VARIANT_OPTIONS = [
  { value: 'default', label: 'Default' },
  { value: 'plan', label: 'Plan First' },
  { value: 'supervised', label: 'Supervised (Ask Permissions)' },
];

export function NewAttemptDialog({
  task,
  open,
  onClose,
  onSubmit,
  isLoading = false,
}: NewAttemptDialogProps) {
  const [executor, setExecutor] = useState('claude-sonnet-4');
  const [variant, setVariant] = useState<string>('default');
  const [prompt, setPrompt] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleSubmit = async () => {
    setIsSubmitting(true);
    try {
      await onSubmit({
        task_id: task.id,
        executor,
        variant: variant !== 'default' ? variant : undefined,
        base_branch: 'main',
        prompt: prompt.trim() || undefined,
      });
      // Reset form and close
      setExecutor('claude-sonnet-4');
      setVariant('default');
      setPrompt('');
      onClose();
    } catch (error) {
      // Error handling left to parent component
      logger.error('Failed to create attempt:', error);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleOpenChange = (newOpen: boolean) => {
    if (!newOpen) {
      onClose();
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Create New Attempt</DialogTitle>
          <DialogDescription>
            Create a new attempt for "{task.title}"
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {/* Executor Selection */}
          <div className="space-y-2">
            <Label htmlFor="executor">Agent Executor</Label>
            <Select value={executor} onValueChange={setExecutor}>
              <SelectTrigger id="executor">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {EXECUTOR_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              Choose which AI agent will work on this task attempt
            </p>
          </div>

          {/* Variant Selection */}
          <div className="space-y-2">
            <Label htmlFor="variant">Variant (Optional)</Label>
            <Select value={variant} onValueChange={setVariant}>
              <SelectTrigger id="variant">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {VARIANT_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              Variant controls agent behavior (permissions, planning, etc.)
            </p>
          </div>

          {/* Additional Instructions */}
          <div className="space-y-2">
            <Label htmlFor="prompt">Additional Instructions (Optional)</Label>
            <Textarea
              id="prompt"
              placeholder={`Any specific guidance for this attempt...
Example:
- Focus on performance optimization
- Use TypeScript strict mode
- Add comprehensive tests`}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              rows={6}
              className="font-mono text-sm resize-none"
            />
            <p className="text-xs text-muted-foreground">
              These instructions will be prepended to the task description
            </p>
          </div>

          {/* Task Description Preview */}
          <div className="rounded-md border p-3 bg-muted/30">
            <Label className="text-xs text-muted-foreground">Task Description</Label>
            <p className="text-sm mt-2 whitespace-pre-wrap line-clamp-6">
              {task.description || '(No description)'}
            </p>
          </div>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={onClose}
            disabled={isSubmitting || isLoading}
          >
            Cancel
          </Button>
          <Button
            onClick={handleSubmit}
            disabled={isSubmitting || isLoading}
          >
            {isSubmitting || isLoading ? 'Creating...' : 'Create Attempt'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
