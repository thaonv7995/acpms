import {
    TASK_DOCUMENT_FORMAT_LABELS,
    TASK_DOCUMENT_KIND_LABELS,
    type TaskDocumentFormat,
    type TaskDocumentKind,
} from '../../../lib/taskDocuments';

interface TaskDocumentFieldsProps {
    documentTitle: string;
    onDocumentTitleChange: (value: string) => void;
    documentKind: TaskDocumentKind;
    onDocumentKindChange: (value: TaskDocumentKind) => void;
    documentFormat: TaskDocumentFormat;
    onDocumentFormatChange: (value: TaskDocumentFormat) => void;
    documentSourceUrl: string;
    onDocumentSourceUrlChange: (value: string) => void;
    documentFigmaUrl: string;
    onDocumentFigmaUrlChange: (value: string) => void;
    documentFigmaNodeId: string;
    onDocumentFigmaNodeIdChange: (value: string) => void;
    documentContent: string;
    onDocumentContentChange: (value: string) => void;
}

const documentKindOptions = Object.entries(TASK_DOCUMENT_KIND_LABELS) as Array<
    [TaskDocumentKind, string]
>;
const documentFormatOptions = Object.entries(TASK_DOCUMENT_FORMAT_LABELS) as Array<
    [TaskDocumentFormat, string]
>;

export function TaskDocumentFields({
    documentTitle,
    onDocumentTitleChange,
    documentKind,
    onDocumentKindChange,
    documentFormat,
    onDocumentFormatChange,
    documentSourceUrl,
    onDocumentSourceUrlChange,
    documentFigmaUrl,
    onDocumentFigmaUrlChange,
    documentFigmaNodeId,
    onDocumentFigmaNodeIdChange,
    documentContent,
    onDocumentContentChange,
}: TaskDocumentFieldsProps) {
    return (
        <div className="space-y-4 rounded-xl border border-border bg-muted/30 p-4">
            <div>
                <h3 className="text-sm font-bold text-card-foreground">Document Metadata</h3>
                <p className="text-xs text-muted-foreground mt-1">
                    Configure how this docs task is previewed and published into the Document Vault.
                </p>
            </div>

            <div>
                <label className="block text-sm font-bold text-card-foreground mb-1.5">
                    Document Title
                </label>
                <input
                    type="text"
                    value={documentTitle}
                    onChange={(event) => onDocumentTitleChange(event.target.value)}
                    placeholder="Defaults to the task title if left blank"
                    className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
                />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                    <label className="block text-sm font-bold text-card-foreground mb-1.5">
                        Document Kind
                    </label>
                    <select
                        value={documentKind}
                        onChange={(event) =>
                            onDocumentKindChange(event.target.value as TaskDocumentKind)
                        }
                        className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                    >
                        {documentKindOptions.map(([value, label]) => (
                            <option key={value} value={value}>
                                {label}
                            </option>
                        ))}
                    </select>
                </div>

                <div>
                    <label className="block text-sm font-bold text-card-foreground mb-1.5">
                        Document Format
                    </label>
                    <select
                        value={documentFormat}
                        onChange={(event) =>
                            onDocumentFormatChange(event.target.value as TaskDocumentFormat)
                        }
                        className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                    >
                        {documentFormatOptions.map(([value, label]) => (
                            <option key={value} value={value}>
                                {label}
                            </option>
                        ))}
                    </select>
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                    <label className="block text-sm font-bold text-card-foreground mb-1.5">
                        Source URL
                    </label>
                    <input
                        type="url"
                        value={documentSourceUrl}
                        onChange={(event) => onDocumentSourceUrlChange(event.target.value)}
                        placeholder="https://example.com/spec"
                        className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
                    />
                </div>

                <div>
                    <label className="block text-sm font-bold text-card-foreground mb-1.5">
                        Figma URL
                    </label>
                    <input
                        type="url"
                        value={documentFigmaUrl}
                        onChange={(event) => onDocumentFigmaUrlChange(event.target.value)}
                        placeholder="https://www.figma.com/design/..."
                        className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
                    />
                </div>
            </div>

            <div>
                <label className="block text-sm font-bold text-card-foreground mb-1.5">
                    Figma Node ID
                </label>
                <input
                    type="text"
                    value={documentFigmaNodeId}
                    onChange={(event) => onDocumentFigmaNodeIdChange(event.target.value)}
                    placeholder="123:456"
                    className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground"
                />
            </div>

            <div>
                <label className="block text-sm font-bold text-card-foreground mb-1.5">
                    Document Content
                </label>
                <textarea
                    value={documentContent}
                    onChange={(event) => onDocumentContentChange(event.target.value)}
                    placeholder="Write the markdown or notes that should become the final document body."
                    rows={6}
                    className="w-full bg-card border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary placeholder-muted-foreground resize-none"
                />
                <p className="mt-1 text-xs text-muted-foreground">
                    Markdown is rendered directly in the docs preview.
                </p>
            </div>
        </div>
    );
}
