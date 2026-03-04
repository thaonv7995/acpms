import { useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface AIDescriptionFieldProps {
    value: string;
    onChange: (value: string) => void;
    onGenerateAI: () => void;
    isGenerating: boolean;
    titleProvided: boolean;
}

export function AIDescriptionField({
    value,
    onChange,
    onGenerateAI,
    isGenerating,
    titleProvided
}: AIDescriptionFieldProps) {
    const [mode, setMode] = useState<'write' | 'preview'>('write');
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    const insertText = (before: string, after: string = '') => {
        const textarea = textareaRef.current;
        if (!textarea) return;

        const start = textarea.selectionStart;
        const end = textarea.selectionEnd;
        const previousValue = textarea.value;
        const selectedText = previousValue.substring(start, end);

        const newValue = previousValue.substring(0, start) +
            before + selectedText + after +
            previousValue.substring(end);

        onChange(newValue);

        // Restore focus and selection
        setTimeout(() => {
            textarea.focus();
            textarea.setSelectionRange(start + before.length, end + before.length);
        }, 0);
    };

    return (
        <div>
            <div className="flex justify-between items-end mb-1.5">
                <div className="flex gap-4">
                    <label className="text-sm font-bold text-card-foreground">Description</label>

                    {/* Tabs */}
                    <div className="flex bg-muted rounded-lg p-0.5">
                        <button
                            onClick={() => setMode('write')}
                            className={`px-3 py-0.5 text-xs font-medium rounded-md transition-all ${mode === 'write'
                                    ? 'bg-card text-card-foreground shadow-sm'
                                    : 'text-muted-foreground hover:text-card-foreground'
                                }`}
                        >
                            Write
                        </button>
                        <button
                            onClick={() => setMode('preview')}
                            className={`px-3 py-0.5 text-xs font-medium rounded-md transition-all ${mode === 'preview'
                                    ? 'bg-card text-card-foreground shadow-sm'
                                    : 'text-muted-foreground hover:text-card-foreground'
                                }`}
                        >
                            Preview
                        </button>
                    </div>
                </div>

                {mode === 'write' && (
                    <div className="flex gap-2">
                        <div className="flex items-center gap-1 bg-muted rounded-md p-0.5">
                            <button onClick={() => insertText('**', '**')} className="p-1 hover:bg-card rounded text-muted-foreground hover:text-card-foreground" title="Bold">
                                <span className="material-symbols-outlined text-[16px]">format_bold</span>
                            </button>
                            <button onClick={() => insertText('*', '*')} className="p-1 hover:bg-card rounded text-muted-foreground hover:text-card-foreground" title="Italic">
                                <span className="material-symbols-outlined text-[16px]">format_italic</span>
                            </button>
                            <button onClick={() => insertText('- ')} className="p-1 hover:bg-card rounded text-muted-foreground hover:text-card-foreground" title="Bullet List">
                                <span className="material-symbols-outlined text-[16px]">format_list_bulleted</span>
                            </button>
                            <button onClick={() => insertText('## ')} className="p-1 hover:bg-card rounded text-muted-foreground hover:text-card-foreground" title="Header">
                                <span className="material-symbols-outlined text-[16px]">title</span>
                            </button>
                            <button onClick={() => insertText('```\n', '\n```')} className="p-1 hover:bg-card rounded text-muted-foreground hover:text-card-foreground" title="Code Block">
                                <span className="material-symbols-outlined text-[16px]">code</span>
                            </button>
                            <div className="w-px h-4 bg-border mx-1"></div>
                            <button onClick={() => insertText('\n\n## Acceptance Criteria\n- [ ] ')} className="p-1 hover:bg-card rounded text-muted-foreground hover:text-card-foreground" title="Add Checklist">
                                <span className="material-symbols-outlined text-[16px]">checklist</span>
                            </button>
                        </div>

                        <button
                            onClick={onGenerateAI}
                            disabled={!titleProvided}
                            className="text-xs font-bold text-primary flex items-center gap-1 hover:underline disabled:opacity-50 disabled:cursor-not-allowed ml-2"
                        >
                            <span className="material-symbols-outlined text-[14px]">auto_awesome</span>
                            Generate with AI
                        </button>
                    </div>
                )}
            </div>

            <div className="relative">
                {mode === 'write' ? (
                    <>
                        <textarea
                            ref={textareaRef}
                            rows={8}
                            value={value}
                            onChange={(e) => onChange(e.target.value)}
                            placeholder="Describe the acceptance criteria and technical details..."
                            className="w-full bg-muted border border-border rounded-lg p-3 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground resize-y font-mono"
                        ></textarea>
                        {isGenerating && (
                            <div className="absolute inset-0 bg-white/50 dark:bg-black/50 backdrop-blur-sm flex items-center justify-center rounded-lg">
                                <div className="flex items-center gap-2 bg-card px-4 py-2 rounded-full shadow-lg border border-border">
                                    <span className="size-2 rounded-full bg-primary animate-ping"></span>
                                    <span className="text-xs font-bold text-card-foreground">AI writing...</span>
                                </div>
                            </div>
                        )}
                    </>
                ) : (
                    <div className="w-full min-h-[192px] bg-muted border border-border rounded-lg p-4 text-sm text-card-foreground overflow-auto prose dark:prose-invert max-w-none">
                        {value ? (
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                {value}
                            </ReactMarkdown>
                        ) : (
                            <span className="text-muted-foreground italic">Nothing to preview</span>
                        )}
                    </div>
                )}
            </div>

            {mode === 'write' && (
                <p className="text-xs text-muted-foreground mt-1 text-right">Markdown supported</p>
            )}
        </div>
    );
}
