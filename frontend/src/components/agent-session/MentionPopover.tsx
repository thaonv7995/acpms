/**
 * MentionPopover - Dropdown for @mention file search
 * Shows searchable list of files when user types @
 */

import { memo, useState, useEffect, useRef, useCallback } from 'react';
import type { MentionItem } from './types';
import { logger } from '@/lib/logger';

interface MentionPopoverProps {
  isOpen: boolean;
  searchQuery: string;
  position: { top: number; left: number };
  onSelect: (item: MentionItem) => void;
  onClose: () => void;
  projectId?: string;
}

// Mock file search - replace with actual API call
async function searchFiles(_projectId: string, query: string): Promise<MentionItem[]> {
  // Simulated file search results
  const mockFiles: MentionItem[] = [
    { type: 'file', value: 'src/App.tsx', display: 'App.tsx', icon: 'description' },
    { type: 'file', value: 'src/main.tsx', display: 'main.tsx', icon: 'description' },
    { type: 'file', value: 'src/components/Header.tsx', display: 'Header.tsx', icon: 'description' },
    { type: 'file', value: 'src/hooks/useAuth.ts', display: 'useAuth.ts', icon: 'code' },
    { type: 'file', value: 'package.json', display: 'package.json', icon: 'data_object' },
    { type: 'file', value: 'README.md', display: 'README.md', icon: 'article' },
    { type: 'file', value: 'tsconfig.json', display: 'tsconfig.json', icon: 'settings' },
    { type: 'file', value: '.env', display: '.env', icon: 'lock' },
  ];

  if (!query) return mockFiles.slice(0, 5);

  const lowerQuery = query.toLowerCase();
  return mockFiles
    .filter(
      (f) =>
        f.value.toLowerCase().includes(lowerQuery) ||
        f.display.toLowerCase().includes(lowerQuery)
    )
    .slice(0, 8);
}

export const MentionPopover = memo(function MentionPopover({
  isOpen,
  searchQuery,
  position,
  onSelect,
  onClose,
  projectId = '',
}: MentionPopoverProps) {
  const [items, setItems] = useState<MentionItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Search files when query changes
  useEffect(() => {
    if (!isOpen) return;

    setLoading(true);
    const timer = setTimeout(async () => {
      try {
        const results = await searchFiles(projectId, searchQuery);
        setItems(results);
        setSelectedIndex(0);
      } catch (error) {
        logger.error('Failed to search files:', error);
        setItems([]);
      } finally {
        setLoading(false);
      }
    }, 150);

    return () => clearTimeout(timer);
  }, [isOpen, searchQuery, projectId]);

  // Handle keyboard navigation
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setSelectedIndex((prev) => Math.min(prev + 1, items.length - 1));
          break;
        case 'ArrowUp':
          e.preventDefault();
          setSelectedIndex((prev) => Math.max(prev - 1, 0));
          break;
        case 'Enter':
          e.preventDefault();
          if (items[selectedIndex]) {
            onSelect(items[selectedIndex]);
          }
          break;
        case 'Escape':
          e.preventDefault();
          onClose();
          break;
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, items, selectedIndex, onSelect, onClose]);

  // Handle click outside
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen, onClose]);

  const handleItemClick = useCallback(
    (item: MentionItem) => {
      onSelect(item);
    },
    [onSelect]
  );

  if (!isOpen) return null;

  return (
    <div
      ref={popoverRef}
      className="absolute z-50 w-72 max-h-64 bg-slate-800 border border-slate-600 rounded-lg shadow-xl overflow-hidden"
      style={{
        top: position.top,
        left: position.left,
      }}
    >
      {/* Header */}
      <div className="px-3 py-2 bg-slate-700/50 border-b border-slate-600 flex items-center gap-2">
        <span className="material-symbols-outlined text-[16px] text-slate-400">search</span>
        <span className="text-xs text-slate-400">
          {searchQuery ? `Search: "${searchQuery}"` : 'Type to search files'}
        </span>
      </div>

      {/* Results */}
      <div className="max-h-48 overflow-y-auto">
        {loading ? (
          <div className="flex items-center justify-center py-4">
            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-primary" />
            <span className="ml-2 text-xs text-slate-500">Searching...</span>
          </div>
        ) : items.length === 0 ? (
          <div className="py-4 text-center text-xs text-slate-500">
            No files found
          </div>
        ) : (
          <div className="py-1">
            {items.map((item, index) => (
              <button
                key={item.value}
                onClick={() => handleItemClick(item)}
                className={`w-full flex items-center gap-2 px-3 py-2 text-left transition-colors ${
                  index === selectedIndex
                    ? 'bg-primary/20 text-slate-200'
                    : 'text-slate-300 hover:bg-slate-700/50'
                }`}
              >
                <span className="material-symbols-outlined text-[16px] text-slate-400">
                  {item.icon}
                </span>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{item.display}</p>
                  <p className="text-xs text-slate-500 truncate">{item.value}</p>
                </div>
                {item.type === 'file' && (
                  <span className="text-xs text-slate-500 px-1.5 py-0.5 bg-slate-700 rounded">
                    file
                  </span>
                )}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Footer hint */}
      <div className="px-3 py-1.5 bg-slate-700/30 border-t border-slate-600 text-xs text-slate-500">
        <kbd className="px-1 py-0.5 bg-slate-700 rounded text-[10px]">Enter</kbd> to select
        <span className="mx-2">|</span>
        <kbd className="px-1 py-0.5 bg-slate-700 rounded text-[10px]">Esc</kbd> to close
      </div>
    </div>
  );
});
