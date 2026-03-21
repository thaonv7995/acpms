import type { TaskType } from '../shared/types';
import type { TaskContext, TaskContextAttachment } from '../api/taskContexts';

export type TaskDocumentKind =
    | 'brainstorm'
    | 'idea_note'
    | 'prd'
    | 'srs'
    | 'design'
    | 'technical_spec'
    | 'research_note'
    | 'meeting_note'
    | 'architecture'
    | 'api_spec'
    | 'database_schema'
    | 'business_rules'
    | 'runbook'
    | 'notes'
    | 'other';

export type TaskDocumentFormat = 'markdown' | 'pdf' | 'image' | 'figma_link' | 'binary';

export interface TaskDocumentMetadata {
    kind: TaskDocumentKind;
    format: TaskDocumentFormat;
    previewMode: 'document';
    publishPolicy: 'final_on_done';
    title: string;
    sourceUrl?: string;
    figmaUrl?: string;
    figmaNodeId?: string;
    vaultDocumentId?: string;
}

const TASK_DOCUMENT_KIND_SET = new Set<TaskDocumentKind>([
    'brainstorm',
    'idea_note',
    'prd',
    'srs',
    'design',
    'technical_spec',
    'research_note',
    'meeting_note',
    'architecture',
    'api_spec',
    'database_schema',
    'business_rules',
    'runbook',
    'notes',
    'other',
]);

const TASK_DOCUMENT_FORMAT_SET = new Set<TaskDocumentFormat>([
    'markdown',
    'pdf',
    'image',
    'figma_link',
    'binary',
]);

function asObject(value: unknown): Record<string, unknown> | null {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
        return null;
    }

    return value as Record<string, unknown>;
}

function readString(value: unknown): string | undefined {
    if (typeof value !== 'string') {
        return undefined;
    }

    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : undefined;
}

export function getTaskDocumentMetadata(
    taskType: TaskType | string | undefined,
    taskTitle: string,
    metadata?: Record<string, unknown> | null,
): TaskDocumentMetadata | null {
    const root = asObject(metadata);
    const document = asObject(root?.document);
    const previewMode = readString(document?.preview_mode);
    const isDocsTask = taskType === 'docs';

    if (!isDocsTask && previewMode !== 'document') {
        return null;
    }

    const figmaUrl = readString(document?.figma_url);
    const formatValue = readString(document?.format);
    const inferredFormat: TaskDocumentFormat = figmaUrl ? 'figma_link' : 'markdown';
    const format = formatValue && TASK_DOCUMENT_FORMAT_SET.has(formatValue as TaskDocumentFormat)
        ? (formatValue as TaskDocumentFormat)
        : inferredFormat;

    const kindValue = readString(document?.kind);
    const kind = kindValue && TASK_DOCUMENT_KIND_SET.has(kindValue as TaskDocumentKind)
        ? (kindValue as TaskDocumentKind)
        : 'other';

    return {
        kind,
        format,
        previewMode: 'document',
        publishPolicy: 'final_on_done',
        title: readString(document?.title) ?? taskTitle,
        sourceUrl: readString(document?.source_url),
        figmaUrl,
        figmaNodeId: readString(document?.figma_node_id),
        vaultDocumentId: readString(document?.vault_document_id),
    };
}

export function isTaskDocumentPreview(
    taskType: TaskType | string | undefined,
    metadata?: Record<string, unknown> | null,
): boolean {
    const root = asObject(metadata);
    const document = asObject(root?.document);
    return taskType === 'docs' || readString(document?.preview_mode) === 'document';
}

export function flattenTaskContextAttachments(
    taskContexts: TaskContext[],
): TaskContextAttachment[] {
    return taskContexts.flatMap((context) => context.attachments ?? []);
}

export function getPrimaryTaskDocumentText(taskContexts: TaskContext[]): string {
    const primary = taskContexts
        .slice()
        .sort((left, right) => {
            if (left.sort_order !== right.sort_order) {
                return left.sort_order - right.sort_order;
            }

            return new Date(left.created_at).getTime() - new Date(right.created_at).getTime();
        })
        .find((context) => context.raw_content.trim().length > 0);

    return primary?.raw_content.trim() ?? '';
}

export function selectTaskDocumentAttachment(
    format: TaskDocumentFormat,
    taskContexts: TaskContext[],
): TaskContextAttachment | null {
    const attachments = flattenTaskContextAttachments(taskContexts);
    return (
        attachments.find((attachment) => {
            if (format === 'pdf') {
                return attachment.content_type === 'application/pdf';
            }

            if (format === 'image') {
                return attachment.content_type.startsWith('image/');
            }

            if (format === 'markdown') {
                return (
                    attachment.content_type.startsWith('text/') ||
                    attachment.content_type === 'application/json' ||
                    attachment.content_type === 'application/yaml' ||
                    attachment.content_type === 'application/x-yaml' ||
                    attachment.content_type === 'application/xml' ||
                    attachment.content_type === 'application/toml'
                );
            }

            if (format === 'binary') {
                return true;
            }

            return false;
        }) ?? null
    );
}

export const TASK_DOCUMENT_KIND_LABELS: Record<TaskDocumentKind, string> = {
    brainstorm: 'Brainstorm',
    idea_note: 'Idea Note',
    prd: 'PRD',
    srs: 'SRS',
    design: 'Design',
    technical_spec: 'Technical Spec',
    research_note: 'Research Note',
    meeting_note: 'Meeting Note',
    architecture: 'Architecture',
    api_spec: 'API Spec',
    database_schema: 'Database Schema',
    business_rules: 'Business Rules',
    runbook: 'Runbook',
    notes: 'Notes',
    other: 'Other',
};

export const TASK_DOCUMENT_FORMAT_LABELS: Record<TaskDocumentFormat, string> = {
    markdown: 'Markdown',
    pdf: 'PDF',
    image: 'Image',
    figma_link: 'Figma Link',
    binary: 'Binary',
};
