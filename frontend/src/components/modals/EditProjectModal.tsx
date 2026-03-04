// EditProjectModal Component
import { useState, useEffect } from 'react';

interface EditProjectModalProps {
    isOpen: boolean;
    onClose: () => void;
    project: {
        id: string;
        name: string;
        description?: string;
    } | null;
    onSave?: (projectId: string, data: { name: string; description: string }) => Promise<void> | void;
}

export function EditProjectModal({ isOpen, onClose, project, onSave }: EditProjectModalProps) {
    const [name, setName] = useState('');
    const [description, setDescription] = useState('');
    const [isSaving, setIsSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (project) {
            setName(project.name);
            setDescription(project.description || '');
            setError(null);
        }
    }, [project]);

    if (!isOpen || !project) return null;

    const handleSave = async () => {
        if (!name.trim()) return;

        setIsSaving(true);
        try {
            await onSave?.(project.id, { name, description });
            onClose();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to save project');
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose}></div>
            <div className="relative w-full max-w-lg bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
                {/* Header */}
                <div className="px-6 py-5 border-b border-border flex justify-between items-center bg-muted">
                    <div>
                        <h2 className="text-lg font-bold text-card-foreground">Edit Project</h2>
                        <p className="text-sm text-muted-foreground">Update project details</p>
                    </div>
                    <button onClick={onClose} className="text-muted-foreground hover:text-card-foreground transition-colors">
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {/* Body */}
                <div className="p-6 flex flex-col gap-4">
                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">Project Name</label>
                        <input
                            type="text"
                            value={name}
                            onChange={(e) => setName(e.target.value)}
                            className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                        />
                    </div>
                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">Description</label>
                        <textarea
                            rows={3}
                            value={description}
                            onChange={(e) => setDescription(e.target.value)}
                            className="w-full bg-muted border border-border rounded-lg p-3 text-sm text-card-foreground focus:ring-primary focus:border-primary resize-none"
                        />
                    </div>
                    {error && (
                        <div className="text-sm text-red-600 dark:text-red-400">{error}</div>
                    )}
                </div>

                {/* Footer */}
                <div className="px-6 py-4 border-t border-border bg-muted flex justify-end gap-3">
                    <button onClick={onClose} className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors">
                        Cancel
                    </button>
                    <button
                        onClick={handleSave}
                        disabled={!name.trim() || isSaving}
                        className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        {isSaving ? 'Saving...' : 'Save Changes'}
                    </button>
                </div>
            </div>
        </div>
    );
}
