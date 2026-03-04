// RequirementFormModal — Create or Edit requirement (detailed form)
import { useState, useEffect, useMemo, useCallback } from 'react';
import type { ChangeEvent, DragEvent } from 'react';
import {
    createRequirement,
    updateRequirement,
    getRequirementAttachmentUploadUrl,
    type Requirement,
    type RequirementStatus,
    type RequirementPriority,
} from '../../api/requirements';

const MAX_FILE_SIZE = 10 * 1024 * 1024; // 10MB
const MAX_FILES = 5;
const ALLOWED_TYPES = [
    'image/png',
    'image/jpeg',
    'image/jpg',
    'image/gif',
    'image/webp',
    'application/pdf',
    'application/msword',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
];

interface PendingAttachment {
    id: string;
    filename: string;
    contentType: string;
    size: number;
    key?: string;
    status: 'uploading' | 'uploaded' | 'failed';
    error?: string;
}

interface AttachmentMeta {
    key: string;
    filename: string;
    content_type: string;
    size: number;
    uploaded_at: string;
}

function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const PRIORITY_OPTIONS: RequirementPriority[] = ['low', 'medium', 'high', 'critical'];
const STATUS_OPTIONS: RequirementStatus[] = ['todo', 'in_progress', 'done'];

const ACCEPTANCE_CRITERIA_MARKER = '\n\n---\nAcceptance criteria:\n';

function parseContent(content: string): { description: string; acceptanceCriteria: string } {
    const idx = content.indexOf(ACCEPTANCE_CRITERIA_MARKER);
    if (idx >= 0) {
        return {
            description: content.slice(0, idx).trim(),
            acceptanceCriteria: content.slice(idx + ACCEPTANCE_CRITERIA_MARKER.length).trim(),
        };
    }
    return { description: content.trim(), acceptanceCriteria: '' };
}

function buildContent(description: string, acceptanceCriteria: string): string {
    const desc = description.trim();
    const criteria = acceptanceCriteria.trim();
    if (!criteria) return desc;
    return desc + ACCEPTANCE_CRITERIA_MARKER + criteria;
}

interface RequirementFormModalProps {
    isOpen: boolean;
    onClose: () => void;
    projectId: string;
    requirement?: Requirement | null; // null = create mode
    onSuccess?: () => void;
}

export function RequirementFormModal({
    isOpen,
    onClose,
    projectId,
    requirement,
    onSuccess,
}: RequirementFormModalProps) {
    const isEdit = Boolean(requirement);

    const [title, setTitle] = useState('');
    const [description, setDescription] = useState('');
    const [acceptanceCriteria, setAcceptanceCriteria] = useState('');
    const [priority, setPriority] = useState<RequirementPriority>('medium');
    const [status, setStatus] = useState<RequirementStatus>('todo');
    const [dueDate, setDueDate] = useState<string>('');
    const [attachments, setAttachments] = useState<PendingAttachment[]>([]);
    const [isSaving, setIsSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [isDragOver, setIsDragOver] = useState(false);

    const uploadedAttachments: AttachmentMeta[] = useMemo(
        () =>
            attachments
                .filter((a): a is PendingAttachment & { key: string } => a.status === 'uploaded' && !!a.key)
                .map((a) => ({
                    key: a.key!,
                    filename: a.filename,
                    content_type: a.contentType,
                    size: a.size,
                    uploaded_at: new Date().toISOString(),
                })),
        [attachments]
    );

    const hasUploadingAttachments = attachments.some((a) => a.status === 'uploading');

    const setAttachmentState = useCallback((id: string, updater: (a: PendingAttachment) => PendingAttachment) => {
        setAttachments((prev) => prev.map((item) => (item.id === id ? updater(item) : item)));
    }, []);

    const uploadFiles = useCallback(
        async (files: File[]) => {
            const valid: File[] = [];
            for (const file of files) {
                if (file.size > MAX_FILE_SIZE) {
                    setError(`File "${file.name}" exceeds 10MB limit`);
                    continue;
                }
                const ct = file.type || 'application/octet-stream';
                if (!ALLOWED_TYPES.includes(ct) && !ct.startsWith('image/')) {
                    setError(`File type not allowed: ${file.name}`);
                    continue;
                }
                valid.push(file);
            }
            if (valid.length === 0) return;
            if (attachments.length + valid.length > MAX_FILES) {
                setError(`Maximum ${MAX_FILES} files allowed`);
                return;
            }

            for (const file of valid) {
                const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
                const contentType = file.type || 'application/octet-stream';

                setAttachments((prev) => [
                    ...prev,
                    { id, filename: file.name, contentType, size: file.size, status: 'uploading' },
                ]);

                try {
                    const { upload_url, key } = await getRequirementAttachmentUploadUrl(projectId, {
                        filename: file.name,
                        content_type: contentType,
                    });

                    const res = await fetch(upload_url, {
                        method: 'PUT',
                        headers: { 'Content-Type': contentType },
                        body: file,
                    });

                    if (!res.ok) throw new Error(`Upload failed: ${res.status}`);

                    setAttachmentState(id, (a) => ({ ...a, key, status: 'uploaded' }));
                } catch (err) {
                    setAttachmentState(id, (a) => ({
                        ...a,
                        status: 'failed',
                        error: err instanceof Error ? err.message : 'Upload failed',
                    }));
                }
            }
        },
        [projectId, attachments.length, setAttachmentState]
    );

    const removeAttachment = useCallback((id: string) => {
        setAttachments((prev) => prev.filter((a) => a.id !== id));
    }, []);

    useEffect(() => {
        if (requirement) {
            setTitle(requirement.title);
            const parsed = parseContent(requirement.content);
            setDescription(parsed.description);
            setAcceptanceCriteria(parsed.acceptanceCriteria);
            setPriority(requirement.priority);
            setStatus(requirement.status);
            setDueDate(requirement.due_date ? requirement.due_date.slice(0, 10) : '');
            const existing = (requirement.metadata?.attachments as AttachmentMeta[] | undefined) || [];
            setAttachments(
                existing.map((a) => ({
                    id: a.key,
                    filename: a.filename,
                    contentType: a.content_type,
                    size: a.size,
                    key: a.key,
                    status: 'uploaded' as const,
                }))
            );
        } else {
            setTitle('');
            setDescription('');
            setAcceptanceCriteria('');
            setPriority('medium');
            setStatus('todo');
            setDueDate('');
            setAttachments([]);
        }
        setError(null);
    }, [requirement, isOpen]);

    if (!isOpen) return null;

    const handleSubmit = async () => {
        if (!title.trim()) {
            setError('Title is required');
            return;
        }
        if (!description.trim()) {
            setError('Description is required');
            return;
        }

        const content = buildContent(description, acceptanceCriteria);
        const metadata =
            requirement != null
                ? { ...requirement.metadata, attachments: uploadedAttachments }
                : uploadedAttachments.length > 0
                  ? { attachments: uploadedAttachments }
                  : undefined;
        setIsSaving(true);
        setError(null);
        try {
            const dueDateVal = dueDate.trim() || undefined;
            if (isEdit && requirement) {
                await updateRequirement(projectId, requirement.id, {
                    title: title.trim(),
                    content,
                    priority,
                    status,
                    due_date: dueDateVal,
                    metadata,
                });
            } else {
                await createRequirement(projectId, {
                    title: title.trim(),
                    content,
                    priority,
                    due_date: dueDateVal,
                    metadata,
                });
            }
            onSuccess?.();
            onClose();
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to save requirement');
        } finally {
            setIsSaving(false);
        }
    };

    const titleLabel = isEdit ? 'Edit Requirement' : 'Create New Requirement';

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px]" onClick={onClose}></div>
            <div className="relative w-full max-w-2xl max-h-[90vh] overflow-hidden bg-card border border-border rounded-2xl shadow-2xl flex flex-col">
                {/* Header */}
                <div className="px-6 py-5 border-b border-border flex justify-between items-center bg-muted shrink-0">
                    <div>
                        <h2 className="text-lg font-bold text-card-foreground">{titleLabel}</h2>
                        <p className="text-sm text-muted-foreground">
                            {isEdit ? 'Update requirement details' : 'Define what needs to be built'}
                        </p>
                    </div>
                    <button onClick={onClose} className="text-muted-foreground hover:text-card-foreground transition-colors">
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {/* Body - scrollable */}
                <div className="p-6 flex-1 overflow-y-auto flex flex-col gap-5">
                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">Title *</label>
                        <input
                            type="text"
                            value={title}
                            onChange={(e) => setTitle(e.target.value)}
                            placeholder="e.g. User authentication and authorization"
                            className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                        />
                    </div>

                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">Description *</label>
                        <p className="text-xs text-muted-foreground mb-1.5">
                            Detailed requirement description, context, scope. Markdown supported.
                        </p>
                        <textarea
                            rows={6}
                            value={description}
                            onChange={(e) => setDescription(e.target.value)}
                            placeholder={`Implement secure login/register flow with JWT tokens. Support email/password and OAuth providers (Google, GitHub).

Context:
- System needs to support both web and mobile clients
- Session timeout: 24h for web, 7 days for mobile`}
                            className="w-full bg-muted border border-border rounded-lg p-3 text-sm text-card-foreground focus:ring-primary focus:border-primary resize-y min-h-[120px]"
                        />
                    </div>

                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">Acceptance Criteria</label>
                        <p className="text-xs text-muted-foreground mb-1.5">
                            Acceptance criteria — one condition per line. Example: &quot;- User can log in with email&quot;
                        </p>
                        <textarea
                            rows={4}
                            value={acceptanceCriteria}
                            onChange={(e) => setAcceptanceCriteria(e.target.value)}
                            placeholder={`- Users can register with email/password
- Users can login and receive JWT
- Token refresh works before expiry
- OAuth (Google, GitHub) login supported`}
                            className="w-full bg-muted border border-border rounded-lg p-3 text-sm text-card-foreground focus:ring-primary focus:border-primary resize-y min-h-[80px] font-mono text-[13px]"
                        />
                    </div>

                    {/* Reference Files */}
                    <div>
                        <label className="block text-sm font-bold text-card-foreground mb-1.5">Reference Files</label>
                        <p className="text-xs text-muted-foreground mb-1.5">
                            PNG, JPG, PDF, DOC — max 10MB, up to {MAX_FILES} files
                        </p>
                        <div
                            onDragOver={(e: DragEvent<HTMLDivElement>) => {
                                e.preventDefault();
                                setIsDragOver(true);
                            }}
                            onDragLeave={() => setIsDragOver(false)}
                            onDrop={async (e: DragEvent<HTMLDivElement>) => {
                                e.preventDefault();
                                setIsDragOver(false);
                                const files = Array.from(e.dataTransfer.files || []);
                                if (files.length) await uploadFiles(files);
                            }}
                            className={`border-2 border-dashed rounded-lg p-6 text-center transition-colors ${
                                isDragOver ? 'border-primary bg-primary/5' : 'border-border bg-muted/30'
                            }`}
                        >
                            <input
                                type="file"
                                id="req-file-input"
                                className="hidden"
                                multiple
                                accept=".png,.jpg,.jpeg,.gif,.webp,.pdf,.doc,.docx"
                                onChange={async (e: ChangeEvent<HTMLInputElement>) => {
                                    const files = Array.from(e.target.files || []);
                                    e.target.value = '';
                                    if (files.length) await uploadFiles(files);
                                }}
                            />
                            <label
                                htmlFor="req-file-input"
                                className="cursor-pointer flex flex-col items-center gap-2 text-muted-foreground hover:text-primary"
                            >
                                <span className="material-symbols-outlined text-3xl">attach_file</span>
                                <span className="text-sm">Drag and drop or click to select file</span>
                            </label>
                        </div>
                        {attachments.length > 0 && (
                            <div className="mt-2 space-y-2">
                                {attachments.map((a) => (
                                    <div
                                        key={a.id}
                                        className="flex items-center justify-between gap-2 p-2 rounded-lg bg-muted/50 border border-border"
                                    >
                                        <div className="flex items-center gap-2 min-w-0">
                                            <span className="material-symbols-outlined text-muted-foreground shrink-0">
                                                {a.status === 'uploaded'
                                                    ? 'check_circle'
                                                    : a.status === 'failed'
                                                      ? 'error'
                                                      : 'progress_activity'}
                                            </span>
                                            <span className="text-sm truncate">{a.filename}</span>
                                            <span className="text-xs text-muted-foreground shrink-0">
                                                {formatBytes(a.size)}
                                            </span>
                                        </div>
                                        <button
                                            type="button"
                                            onClick={() => removeAttachment(a.id)}
                                            className="text-muted-foreground hover:text-destructive shrink-0"
                                        >
                                            <span className="material-symbols-outlined text-[18px]">close</span>
                                        </button>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>

                    <div className="flex flex-wrap gap-4">
                        <div className="flex-1 min-w-[140px]">
                            <label className="block text-sm font-bold text-card-foreground mb-1.5">Priority</label>
                            <select
                                value={priority}
                                onChange={(e) => setPriority(e.target.value as RequirementPriority)}
                                className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                            >
                                {PRIORITY_OPTIONS.map((p) => (
                                    <option key={p} value={p}>
                                        {p.charAt(0).toUpperCase() + p.slice(1)}
                                    </option>
                                ))}
                            </select>
                        </div>
                        <div className="flex-1 min-w-[140px]">
                            <label className="block text-sm font-bold text-card-foreground mb-1.5">Due Date</label>
                            <input
                                type="date"
                                value={dueDate}
                                onChange={(e) => setDueDate(e.target.value)}
                                className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                            />
                        </div>
                        {isEdit && (
                            <div className="flex-1 min-w-[140px]">
                                <label className="block text-sm font-bold text-card-foreground mb-1.5">Status</label>
                                <select
                                    value={status}
                                    onChange={(e) => setStatus(e.target.value as RequirementStatus)}
                                    className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                                >
                                    {STATUS_OPTIONS.map((s) => (
                                        <option key={s} value={s}>
                                            {s === 'in_progress' ? 'In Progress' : s.charAt(0).toUpperCase() + s.slice(1)}
                                        </option>
                                    ))}
                                </select>
                            </div>
                        )}
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
                        onClick={handleSubmit}
                        disabled={!title.trim() || !description.trim() || isSaving || hasUploadingAttachments}
                        className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        {isSaving ? 'Saving...' : isEdit ? 'Save Changes' : 'Create Requirement'}
                    </button>
                </div>
            </div>
        </div>
    );
}
