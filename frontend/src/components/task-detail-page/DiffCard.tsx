import { useMemo, useState } from 'react';
import { DiffView, DiffModeEnum } from '@git-diff-view/react';
import { generateDiffFile } from '@git-diff-view/file';
import { FileDiff, DiffChangeType } from '../../api/taskAttempts';
import { logger } from '@/lib/logger';

// Language detection based on file extension
function getLanguageFromPath(path: string): string {
    const ext = path.split('.').pop()?.toLowerCase() || '';
    const langMap: Record<string, string> = {
        ts: 'typescript',
        tsx: 'tsx',
        js: 'javascript',
        jsx: 'jsx',
        py: 'python',
        rs: 'rust',
        go: 'go',
        java: 'java',
        rb: 'ruby',
        php: 'php',
        css: 'css',
        scss: 'scss',
        html: 'html',
        json: 'json',
        yaml: 'yaml',
        yml: 'yaml',
        md: 'markdown',
        sql: 'sql',
        sh: 'bash',
        toml: 'toml',
    };
    return langMap[ext] || 'plaintext';
}

// Icon for change type
function ChangeIcon({ change }: { change: DiffChangeType }) {
    const icons: Record<DiffChangeType, { icon: string; color: string }> = {
        added: { icon: 'add_circle', color: 'text-green-500' },
        deleted: { icon: 'remove_circle', color: 'text-red-500' },
        modified: { icon: 'edit', color: 'text-blue-500' },
        renamed: { icon: 'swap_horiz', color: 'text-yellow-500' },
    };
    const { icon, color } = icons[change] || icons.modified;
    return <span className={`material-symbols-outlined text-[16px] ${color}`}>{icon}</span>;
}

interface DiffCardProps {
    diff: FileDiff;
    defaultExpanded?: boolean;
}

export function DiffCard({ diff, defaultExpanded = true }: DiffCardProps) {
    const [expanded, setExpanded] = useState(defaultExpanded);

    const filePath = diff.new_path || diff.old_path || 'unknown';
    const language = getLanguageFromPath(filePath);

    // Generate diff file for the viewer
    const diffFile = useMemo(() => {
        const oldContent = diff.old_content || '';
        const newContent = diff.new_content || '';

        // Skip if no content
        if (!oldContent && !newContent) return null;

        try {
            const file = generateDiffFile(
                diff.old_path || filePath,
                oldContent,
                diff.new_path || filePath,
                newContent,
                language,
                language
            );
            file.initRaw();
            return file;
        } catch (e) {
            logger.error('Failed to generate diff:', e);
            return null;
        }
    }, [diff, filePath, language]);

    return (
        <div className="border border-slate-200 dark:border-slate-700 rounded-lg overflow-hidden mb-3">
            {/* Header */}
            <button
                onClick={() => setExpanded(!expanded)}
                className="w-full px-4 py-2.5 bg-slate-50 dark:bg-slate-800/50 flex items-center gap-3 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
            >
                <span className={`material-symbols-outlined text-[14px] text-slate-400 transition-transform ${expanded ? 'rotate-90' : ''}`}>
                    chevron_right
                </span>
                <ChangeIcon change={diff.change} />
                <span className="text-sm font-mono text-slate-700 dark:text-slate-300 flex-1 text-left truncate">
                    {diff.change === 'renamed' && diff.old_path ? (
                        <>
                            <span className="text-slate-500">{diff.old_path}</span>
                            <span className="mx-2 text-slate-400">→</span>
                            <span>{diff.new_path}</span>
                        </>
                    ) : (
                        filePath
                    )}
                </span>
                <div className="flex items-center gap-2 text-xs">
                    <span className="text-green-500">+{diff.additions}</span>
                    <span className="text-red-500">-{diff.deletions}</span>
                </div>
            </button>

            {/* Diff Content */}
            {expanded && (
                <div className="border-t border-slate-200 dark:border-slate-700">
                    {diffFile ? (
                        <DiffView
                            diffFile={diffFile}
                            diffViewWrap={false}
                            diffViewTheme="dark"
                            diffViewHighlight
                            diffViewMode={DiffModeEnum.Unified}
                            diffViewFontSize={12}
                        />
                    ) : (
                        <div className="p-4 text-sm text-slate-500 text-center">
                            {diff.change === 'deleted' ? 'File deleted' :
                             diff.change === 'added' && !diff.new_content ? 'Binary file or empty content' :
                             'No diff available'}
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
