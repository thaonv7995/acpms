type ConfigMode = 'ai' | 'manual';

interface ManualProjectFormProps {
    projectName: string;
    requirements: string;
    configMode: ConfigMode;
    visibility: 'private' | 'public' | 'internal';
    onProjectNameChange: (name: string) => void;
    onRequirementsChange: (requirements: string) => void;
    onConfigModeChange: (mode: ConfigMode) => void;
    onVisibilityChange: (visibility: 'private' | 'public' | 'internal') => void;
}

export function ManualProjectForm({
    projectName,
    requirements,
    configMode,
    visibility,
    onProjectNameChange,
    onRequirementsChange,
    onConfigModeChange,
    onVisibilityChange
}: ManualProjectFormProps) {
    return (
        <div className="flex flex-col gap-6">
            <div>
                <label className="block text-sm font-bold text-card-foreground mb-1.5">Project Name</label>
                <input
                    type="text"
                    value={projectName}
                    onChange={(e) => onProjectNameChange(e.target.value)}
                    placeholder="My Project"
                    className="w-full bg-muted border border-border rounded-lg py-2.5 px-4 text-card-foreground focus:ring-primary focus:border-primary"
                />
            </div>

            <div>
                <label className="block text-sm font-bold text-card-foreground mb-1.5">GitLab Visibility</label>
                <select
                    value={visibility}
                    onChange={(e) => onVisibilityChange(e.target.value as 'private' | 'public' | 'internal')}
                    className="w-full bg-muted border border-border rounded-lg py-2.5 px-4 text-card-foreground focus:ring-primary focus:border-primary"
                >
                    <option value="private">Private - Only accessible to project members</option>
                    <option value="internal">Internal - Accessible to all logged-in users</option>
                    <option value="public">Public - Accessible to everyone</option>
                </select>
            </div>

            <div className="flex bg-muted p-1 rounded-lg self-center">
                <button
                    onClick={() => onConfigModeChange('ai')}
                    className={`px-4 py-2 rounded-md text-sm font-bold flex items-center gap-2 transition-all ${configMode === 'ai'
                        ? 'bg-card text-primary shadow-sm'
                        : 'text-muted-foreground hover:text-card-foreground'
                        }`}
                >
                    <span className="material-symbols-outlined text-[18px]">auto_fix</span>
                    AI Architect
                </button>
                <button
                    onClick={() => onConfigModeChange('manual')}
                    className={`px-4 py-2 rounded-md text-sm font-bold flex items-center gap-2 transition-all ${configMode === 'manual'
                        ? 'bg-card text-primary shadow-sm'
                        : 'text-muted-foreground hover:text-card-foreground'
                        }`}
                >
                    <span className="material-symbols-outlined text-[18px]">tune</span>
                    Manual Config
                </button>
            </div>

            {configMode === 'ai' ? (
                <div>
                    <label className="block text-sm font-bold text-card-foreground mb-2">
                        Describe your project requirements
                    </label>
                    <div className="relative">
                        <textarea
                            className="w-full h-40 bg-muted border border-border rounded-lg p-4 text-sm text-card-foreground focus:ring-primary focus:border-primary resize-none placeholder-muted-foreground"
                            placeholder="E.g. I need a SaaS dashboard for inventory management. It should handle real-time stock updates via websocket, have role-based auth, and use a relational database."
                            value={requirements}
                            onChange={(e) => onRequirementsChange(e.target.value)}
                        ></textarea>
                        <div className="absolute bottom-3 right-3 flex items-center gap-2 text-xs text-muted-foreground bg-card px-2 py-1 rounded border border-border shadow-sm">
                            <span className="material-symbols-outlined text-[14px] text-primary">smart_toy</span>
                            AI will suggest the best stack
                        </div>
                    </div>
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div>
                        <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">Frontend Framework</label>
                        <select className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary">
                            <option>React</option>
                            <option>Vue.js</option>
                            <option>Angular</option>
                            <option>Next.js</option>
                        </select>
                    </div>
                    <div>
                        <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">Backend Runtime</label>
                        <select className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary">
                            <option>Node.js</option>
                            <option>Python (FastAPI)</option>
                            <option>Go</option>
                        </select>
                    </div>
                    <div>
                        <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">Database</label>
                        <select className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary">
                            <option>PostgreSQL</option>
                            <option>MongoDB</option>
                            <option>MySQL</option>
                        </select>
                    </div>
                    <div>
                        <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">State Management</label>
                        <select className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary">
                            <option>Zustand</option>
                            <option>Redux Toolkit</option>
                            <option>Context API</option>
                        </select>
                    </div>
                </div>
            )}
        </div>
    );
}
