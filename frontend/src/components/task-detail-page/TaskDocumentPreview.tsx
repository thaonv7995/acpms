import { useEffect, useMemo, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Task } from '../../api/tasks';
import type { TaskContext, TaskContextAttachment } from '../../api/taskContexts';
import {
    getTaskContextAttachmentDownloadUrl,
} from '../../api/taskContexts';
import {
    getProjectDocument,
    getProjectDocumentDownloadUrl,
    type ProjectDocument,
} from '../../api/projectDocuments';
import {
    getPrimaryTaskDocumentText,
    getTaskDocumentMetadata,
    selectTaskDocumentAttachment,
    TASK_DOCUMENT_FORMAT_LABELS,
    TASK_DOCUMENT_KIND_LABELS,
    type TaskDocumentFormat,
} from '../../lib/taskDocuments';

interface TaskDocumentPreviewProps {
    task: Task;
    taskContexts: TaskContext[];
    metadata?: Record<string, unknown>;
    isReviewMode?: boolean;
}

interface LoadedAssetState {
    assetUrl: string | null;
    inlineText: string | null;
    sourceLabel: string | null;
    error: string | null;
}

async function loadTaskAttachmentUrl(
    taskId: string,
    attachment: TaskContextAttachment,
): Promise<string> {
    const response = await getTaskContextAttachmentDownloadUrl(taskId, attachment.storage_key);
    return response.download_url;
}

async function loadProjectDocumentAsset(
    projectId: string,
    documentId: string,
): Promise<{ document: ProjectDocument; downloadUrl: string }> {
    const document = await getProjectDocument(projectId, documentId);
    const { download_url } = await getProjectDocumentDownloadUrl(projectId, document.storage_key);
    return { document, downloadUrl: download_url };
}

async function fetchTextPreview(downloadUrl: string): Promise<string> {
    const response = await fetch(downloadUrl);
    if (!response.ok) {
        throw new Error(`Failed to fetch document content (${response.status})`);
    }

    return response.text();
}

function resolveAssetSourceLabel(
    attachment: TaskContextAttachment | null,
    projectDocument: ProjectDocument | null,
): string | null {
    if (attachment) {
        return attachment.filename;
    }

    if (projectDocument) {
        return projectDocument.filename;
    }

    return null;
}

function shouldLoadTextPreview(format: TaskDocumentFormat, initialText: string): boolean {
    return format === 'markdown' && initialText.trim().length === 0;
}

export function TaskDocumentPreview({
    task,
    taskContexts,
    metadata,
    isReviewMode = false,
}: TaskDocumentPreviewProps) {
    const documentMetadata = useMemo(
        () => getTaskDocumentMetadata(task.task_type, task.title, metadata ?? task.metadata),
        [metadata, task.metadata, task.task_type, task.title],
    );
    const initialText = useMemo(() => getPrimaryTaskDocumentText(taskContexts), [taskContexts]);
    const attachment = useMemo(
        () => (documentMetadata ? selectTaskDocumentAttachment(documentMetadata.format, taskContexts) : null),
        [documentMetadata, taskContexts],
    );

    const [loadedAsset, setLoadedAsset] = useState<LoadedAssetState>({
        assetUrl: null,
        inlineText: null,
        sourceLabel: null,
        error: null,
    });
    const [loadingAsset, setLoadingAsset] = useState(false);

    useEffect(() => {
        let cancelled = false;

        async function loadAsset() {
            if (!documentMetadata) {
                setLoadedAsset({
                    assetUrl: null,
                    inlineText: null,
                    sourceLabel: null,
                    error: null,
                });
                return;
            }

            const needsAsset =
                documentMetadata.format !== 'figma_link' &&
                (documentMetadata.format !== 'markdown' || shouldLoadTextPreview(documentMetadata.format, initialText));
            if (!needsAsset && !attachment && !documentMetadata.vaultDocumentId) {
                setLoadedAsset({
                    assetUrl: null,
                    inlineText: null,
                    sourceLabel: null,
                    error: null,
                });
                return;
            }

            setLoadingAsset(true);
            try {
                let assetUrl: string | null = null;
                let inlineText: string | null = null;
                let projectDocument: ProjectDocument | null = null;

                if (attachment) {
                    assetUrl = await loadTaskAttachmentUrl(task.id, attachment);
                } else if (documentMetadata.vaultDocumentId) {
                    const loadedDocument = await loadProjectDocumentAsset(
                        task.project_id,
                        documentMetadata.vaultDocumentId,
                    );
                    projectDocument = loadedDocument.document;
                    assetUrl = loadedDocument.downloadUrl;
                }

                if (assetUrl && shouldLoadTextPreview(documentMetadata.format, initialText)) {
                    inlineText = await fetchTextPreview(assetUrl);
                }

                if (!cancelled) {
                    setLoadedAsset({
                        assetUrl,
                        inlineText,
                        sourceLabel: resolveAssetSourceLabel(attachment, projectDocument),
                        error: null,
                    });
                }
            } catch (error) {
                if (!cancelled) {
                    setLoadedAsset({
                        assetUrl: null,
                        inlineText: null,
                        sourceLabel: null,
                        error: error instanceof Error ? error.message : 'Failed to load document preview.',
                    });
                }
            } finally {
                if (!cancelled) {
                    setLoadingAsset(false);
                }
            }
        }

        void loadAsset();

        return () => {
            cancelled = true;
        };
    }, [
        attachment,
        documentMetadata,
        initialText,
        task.id,
        task.project_id,
    ]);

    if (!documentMetadata) {
        return null;
    }

    const displayText = initialText || loadedAsset.inlineText || '';
    const title = documentMetadata.title || task.title;
    const badgeTone = isReviewMode ? 'text-amber-300 bg-amber-500/10 border-amber-500/20' : 'text-primary bg-primary/10 border-primary/20';

    return (
        <div className="bg-card border border-border rounded-xl p-6 space-y-4">
            <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                <div className="space-y-2">
                    <div className="flex flex-wrap items-center gap-2">
                        <span className={`inline-flex items-center rounded-full border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-wide ${badgeTone}`}>
                            {isReviewMode ? 'Document Review' : 'Document Preview'}
                        </span>
                        <span className="inline-flex items-center rounded-full border border-border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                            {TASK_DOCUMENT_KIND_LABELS[documentMetadata.kind]}
                        </span>
                        <span className="inline-flex items-center rounded-full border border-border px-2.5 py-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                            {TASK_DOCUMENT_FORMAT_LABELS[documentMetadata.format]}
                        </span>
                    </div>
                    <div>
                        <h3 className="text-lg font-semibold text-card-foreground">{title}</h3>
                        {task.description && (
                            <p className="mt-1 text-sm text-muted-foreground">{task.description}</p>
                        )}
                    </div>
                </div>

                <div className="flex flex-wrap gap-2">
                    {documentMetadata.figmaUrl && (
                        <a
                            href={documentMetadata.figmaUrl}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-muted/50 px-3 py-2 text-xs font-medium text-card-foreground hover:bg-muted"
                        >
                            <span className="material-symbols-outlined text-[16px]">open_in_new</span>
                            Open Figma
                        </a>
                    )}
                    {loadedAsset.assetUrl && documentMetadata.format !== 'markdown' && (
                        <a
                            href={loadedAsset.assetUrl}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-muted/50 px-3 py-2 text-xs font-medium text-card-foreground hover:bg-muted"
                        >
                            <span className="material-symbols-outlined text-[16px]">download</span>
                            Open File
                        </a>
                    )}
                </div>
            </div>

            {(documentMetadata.sourceUrl || documentMetadata.figmaNodeId || loadedAsset.sourceLabel) && (
                <div className="rounded-lg border border-border bg-muted/30 p-4 text-xs text-muted-foreground space-y-1">
                    {loadedAsset.sourceLabel && (
                        <p>
                            <span className="font-semibold text-card-foreground">Asset:</span>{' '}
                            {loadedAsset.sourceLabel}
                        </p>
                    )}
                    {documentMetadata.sourceUrl && (
                        <p className="break-all">
                            <span className="font-semibold text-card-foreground">Source URL:</span>{' '}
                            <a
                                href={documentMetadata.sourceUrl}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="text-primary hover:underline"
                            >
                                {documentMetadata.sourceUrl}
                            </a>
                        </p>
                    )}
                    {documentMetadata.figmaNodeId && (
                        <p>
                            <span className="font-semibold text-card-foreground">Figma Node:</span>{' '}
                            <code>{documentMetadata.figmaNodeId}</code>
                        </p>
                    )}
                </div>
            )}

            {loadedAsset.error && (
                <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">
                    {loadedAsset.error}
                </div>
            )}

            {loadingAsset && (
                <div className="rounded-lg border border-border bg-muted/30 px-4 py-3 text-sm text-muted-foreground">
                    Loading document preview...
                </div>
            )}

            {documentMetadata.format === 'markdown' && (
                <div className="rounded-xl border border-border bg-muted/20 p-5">
                    {displayText ? (
                        <div className="prose prose-invert max-w-none prose-headings:text-card-foreground prose-p:text-card-foreground/90 prose-strong:text-card-foreground prose-a:text-primary prose-code:text-card-foreground">
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{displayText}</ReactMarkdown>
                        </div>
                    ) : (
                        <p className="text-sm text-muted-foreground">
                            No markdown content has been attached to this document yet.
                        </p>
                    )}
                </div>
            )}

            {documentMetadata.format === 'pdf' && loadedAsset.assetUrl && (
                <div className="overflow-hidden rounded-xl border border-border bg-black/20">
                    <iframe
                        src={loadedAsset.assetUrl}
                        title={title}
                        className="h-[70vh] w-full bg-white"
                    />
                </div>
            )}

            {documentMetadata.format === 'image' && loadedAsset.assetUrl && (
                <div className="overflow-hidden rounded-xl border border-border bg-black/20">
                    <img
                        src={loadedAsset.assetUrl}
                        alt={title}
                        className="max-h-[70vh] w-full object-contain bg-black/40"
                    />
                </div>
            )}

            {documentMetadata.format === 'figma_link' && (
                <div className="rounded-xl border border-border bg-muted/20 p-5">
                    <div className="flex items-start gap-4">
                        <div className="rounded-xl bg-pink-500/10 p-3">
                            <span className="material-symbols-outlined text-pink-400">design_services</span>
                        </div>
                        <div className="space-y-2">
                            <p className="text-sm font-semibold text-card-foreground">
                                Figma design reference
                            </p>
                            <p className="text-sm text-muted-foreground">
                                This docs task links to a Figma source. V1 preview shows the link and metadata only.
                            </p>
                            {documentMetadata.figmaUrl ? (
                                <a
                                    href={documentMetadata.figmaUrl}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center gap-1.5 rounded-lg bg-pink-500 px-3 py-2 text-xs font-semibold text-white hover:bg-pink-600"
                                >
                                    <span className="material-symbols-outlined text-[16px]">open_in_new</span>
                                    Open in Figma
                                </a>
                            ) : (
                                <p className="text-sm text-muted-foreground">
                                    No Figma URL is attached yet.
                                </p>
                            )}
                        </div>
                    </div>
                </div>
            )}

            {documentMetadata.format === 'binary' && (
                <div className="rounded-xl border border-border bg-muted/20 p-5">
                    <div className="flex items-start gap-4">
                        <div className="rounded-xl bg-sky-500/10 p-3">
                            <span className="material-symbols-outlined text-sky-400">draft</span>
                        </div>
                        <div className="space-y-2">
                            <p className="text-sm font-semibold text-card-foreground">
                                Binary or unsupported document
                            </p>
                            <p className="text-sm text-muted-foreground">
                                Download or open the attached file to review the final artifact.
                            </p>
                            {loadedAsset.assetUrl ? (
                                <a
                                    href={loadedAsset.assetUrl}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center gap-1.5 rounded-lg bg-sky-500 px-3 py-2 text-xs font-semibold text-white hover:bg-sky-600"
                                >
                                    <span className="material-symbols-outlined text-[16px]">download</span>
                                    Open File
                                </a>
                            ) : (
                                <p className="text-sm text-muted-foreground">
                                    No downloadable file is attached yet.
                                </p>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
