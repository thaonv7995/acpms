// Diff utilities and configuration
import type { DiffStatus } from '../../types/diff';

export interface DiffStatusConfig {
  color: string;
  bgColor: string;
  icon: string;
  label: string;
}

export const statusConfig: Record<DiffStatus, DiffStatusConfig> = {
  added: {
    color: 'text-green-500',
    bgColor: 'bg-green-50 dark:bg-green-900/20',
    icon: 'add_circle',
    label: 'Added',
  },
  modified: {
    color: 'text-yellow-500',
    bgColor: 'bg-yellow-50 dark:bg-yellow-900/20',
    icon: 'edit',
    label: 'Modified',
  },
  deleted: {
    color: 'text-red-500',
    bgColor: 'bg-red-50 dark:bg-red-900/20',
    icon: 'remove_circle',
    label: 'Deleted',
  },
  renamed: {
    color: 'text-blue-500',
    bgColor: 'bg-blue-50 dark:bg-blue-900/20',
    icon: 'drive_file_rename_outline',
    label: 'Renamed',
  },
};

// Language mapping for syntax highlighting
const langMap: Record<string, string> = {
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  py: 'python',
  rs: 'rust',
  go: 'go',
  java: 'java',
  kt: 'kotlin',
  swift: 'swift',
  rb: 'ruby',
  php: 'php',
  css: 'css',
  scss: 'scss',
  less: 'less',
  html: 'html',
  vue: 'vue',
  svelte: 'svelte',
  json: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  md: 'markdown',
  sql: 'sql',
  sh: 'bash',
  bash: 'bash',
  dockerfile: 'dockerfile',
};

export function getLanguageFromPath(filePath: string): string {
  const ext = filePath.split('.').pop()?.toLowerCase() || '';
  return langMap[ext] || 'plaintext';
}

export function parseFilePath(filePath: string): { fileName: string; dirPath: string } {
  const fileName = filePath.split('/').pop() || filePath;
  const dirPath = filePath.includes('/')
    ? filePath.substring(0, filePath.lastIndexOf('/'))
    : '';
  return { fileName, dirPath };
}
