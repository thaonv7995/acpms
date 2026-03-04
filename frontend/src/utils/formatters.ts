/**
 * Format ISO timestamp to human-readable string
 * Examples:
 * - "just now"
 * - "5 minutes ago"
 * - "Jan 10, 2:45 PM"
 * - "Yesterday at 3:30 PM"
 */
export function formatTimestamp(
  timestamp: string,
  options?: { relative?: boolean }
): string {
  const { relative = true } = options || {};

  try {
    const date = new Date(timestamp);

    if (!relative) {
      return date.toLocaleString('en-US', {
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
        hour12: true,
      });
    }

    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSecs = Math.floor(diffMs / 1000);
    const diffMins = Math.floor(diffSecs / 60);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffSecs < 60) {
      return 'just now';
    }
    if (diffMins < 60) {
      return `${diffMins} minute${diffMins > 1 ? 's' : ''} ago`;
    }
    if (diffHours < 24) {
      return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
    }
    if (diffDays === 1) {
      return `Yesterday at ${date.toLocaleString('en-US', {
        hour: 'numeric',
        minute: '2-digit',
        hour12: true,
      })}`;
    }
    if (diffDays < 7) {
      return `${diffDays} days ago`;
    }

    return date.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
      hour: 'numeric',
      minute: '2-digit',
      hour12: true,
    });
  } catch (error) {
    return timestamp;
  }
}

/**
 * Truncate text with ellipsis
 */
export function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + '...';
}

/**
 * Format byte size to human-readable format
 * Examples: 1.2 KB, 5.4 MB, 1.1 GB
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';

  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

/**
 * Format exit code with label
 */
export function formatExitCode(code: number): string {
  if (code === 0) return 'Success (0)';
  if (code === 1) return 'Error (1)';
  return `Exit ${code}`;
}
