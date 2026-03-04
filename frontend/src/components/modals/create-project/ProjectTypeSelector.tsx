interface ProjectTypeSelectorProps {
    onSelectImport: () => void;
    onSelectNew: () => void;
}

export function ProjectTypeSelector({ onSelectImport, onSelectNew }: ProjectTypeSelectorProps) {
    return (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <button
                onClick={onSelectImport}
                className="group flex flex-col items-center justify-center p-8 rounded-xl bg-white dark:bg-[#161b22] border border-slate-200 dark:border-slate-700 hover:border-primary dark:hover:border-primary hover:bg-slate-50 dark:hover:bg-[#1c2128] transition-all"
            >
                <div className="size-16 rounded-full bg-[#FC6D26]/10 flex items-center justify-center mb-4 group-hover:scale-110 transition-transform">
                    <span className="material-symbols-outlined text-[#FC6D26] text-4xl">code</span>
                </div>
                <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-2">Import from GitLab or GitHub</h3>
                <p className="text-sm text-slate-500 dark:text-slate-400 text-center">Connect an existing repository. If ACPMS cannot push, you can fork it to your own account.</p>
            </button>

            <button
                onClick={onSelectNew}
                className="group flex flex-col items-center justify-center p-8 rounded-xl bg-white dark:bg-[#161b22] border border-slate-200 dark:border-slate-700 hover:border-primary dark:hover:border-primary hover:bg-slate-50 dark:hover:bg-[#1c2128] transition-all"
            >
                <div className="size-16 rounded-full bg-primary/10 flex items-center justify-center mb-4 group-hover:scale-110 transition-transform">
                    <span className="material-symbols-outlined text-primary text-4xl">add_circle</span>
                </div>
                <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-2">Create New Project</h3>
                <p className="text-sm text-slate-500 dark:text-slate-400 text-center">Start from scratch. Choose your stack or let AI decide.</p>
            </button>
        </div>
    );
}
