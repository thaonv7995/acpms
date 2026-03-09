import { useEffect, useMemo, useRef, useState, type ChangeEvent } from 'react';
import {
  createProjectDocument,
  deleteProjectDocument,
  getProjectDocumentDownloadUrl,
  getProjectDocuments,
  getProjectDocumentUploadUrl,
  type ProjectDocument,
  type ProjectDocumentKind,
  updateProjectDocument,
} from '../../api/projectDocuments';
import { ConfirmModal } from '../modals';
import { logger } from '@/lib/logger';

interface DocumentsTabProps {
  projectId: string;
}

interface DocumentDraft {
  documentId: string | null;
  title: string;
  filename: string;
  documentKind: ProjectDocumentKind;
  contentType: string;
  content: string;
}

const DOCUMENT_KIND_LABELS: Record<ProjectDocumentKind, string> = {
  architecture: 'Architecture',
  api_spec: 'API Spec',
  database_schema: 'Database Schema',
  business_rules: 'Business Rules',
  runbook: 'Runbook',
  notes: 'Notes',
  other: 'Other',
};

const CONTENT_TYPE_SUGGESTIONS = [
  { value: 'text/markdown', label: 'Markdown' },
  { value: 'text/plain', label: 'Plain text' },
  { value: 'application/json', label: 'JSON' },
  { value: 'application/vnd.api+json', label: 'JSON API / +json' },
  { value: 'application/yaml', label: 'YAML' },
  { value: 'application/x-yaml', label: 'YAML (x-yaml)' },
  { value: 'text/html', label: 'HTML' },
  { value: 'application/xml', label: 'XML' },
  { value: 'text/csv', label: 'CSV' },
  { value: 'text/css', label: 'CSS' },
  { value: 'text/javascript', label: 'JavaScript' },
  { value: 'application/typescript', label: 'TypeScript' },
  { value: 'text/jsx', label: 'JSX' },
  { value: 'text/tsx', label: 'TSX' },
  { value: 'application/graphql', label: 'GraphQL' },
  { value: 'application/toml', label: 'TOML' },
  { value: 'text/sql', label: 'SQL' },
  { value: 'text/x-shellscript', label: 'Shell script' },
  { value: 'text/x-python', label: 'Python' },
  { value: 'text/x-rust', label: 'Rust' },
] as const;

const IMPORT_FILE_ACCEPT = [
  '.md',
  '.markdown',
  '.txt',
  '.json',
  '.yaml',
  '.yml',
  '.html',
  '.htm',
  '.xml',
  '.csv',
  '.css',
  '.js',
  '.mjs',
  '.cjs',
  '.ts',
  '.tsx',
  '.jsx',
  '.graphql',
  '.gql',
  '.toml',
  '.sql',
  '.sh',
  '.bash',
  '.zsh',
  '.py',
  '.rs',
  '.env',
  '.ini',
  '.cfg',
  '.conf',
].join(',');

function createEmptyDraft(): DocumentDraft {
  return {
    documentId: null,
    title: '',
    filename: '',
    documentKind: 'notes',
    contentType: 'text/markdown',
    content: '',
  };
}

function guessContentType(filename: string, fallback?: string): string {
  const normalized = filename.trim().toLowerCase();
  if (normalized.endsWith('.md') || normalized.endsWith('.markdown')) return 'text/markdown';
  if (normalized.endsWith('.txt')) return 'text/plain';
  if (normalized.endsWith('.json')) return 'application/json';
  if (normalized.endsWith('.yaml')) return 'application/yaml';
  if (normalized.endsWith('.yml')) return 'application/x-yaml';
  if (normalized.endsWith('.html') || normalized.endsWith('.htm')) return 'text/html';
  if (normalized.endsWith('.xml')) return 'application/xml';
  if (normalized.endsWith('.csv')) return 'text/csv';
  if (normalized.endsWith('.css')) return 'text/css';
  if (normalized.endsWith('.js') || normalized.endsWith('.mjs') || normalized.endsWith('.cjs')) {
    return 'text/javascript';
  }
  if (normalized.endsWith('.ts')) return 'application/typescript';
  if (normalized.endsWith('.tsx')) return 'text/tsx';
  if (normalized.endsWith('.jsx')) return 'text/jsx';
  if (normalized.endsWith('.graphql') || normalized.endsWith('.gql')) return 'application/graphql';
  if (normalized.endsWith('.toml')) return 'application/toml';
  if (normalized.endsWith('.sql')) return 'text/sql';
  if (
    normalized.endsWith('.sh') ||
    normalized.endsWith('.bash') ||
    normalized.endsWith('.zsh')
  ) {
    return 'text/x-shellscript';
  }
  if (normalized.endsWith('.py')) return 'text/x-python';
  if (normalized.endsWith('.rs')) return 'text/x-rust';
  if (
    normalized.endsWith('.env') ||
    normalized.endsWith('.ini') ||
    normalized.endsWith('.cfg') ||
    normalized.endsWith('.conf')
  ) {
    return 'text/plain';
  }
  if (fallback && fallback.trim()) return fallback;
  return 'text/markdown';
}

function stripFilenameExtension(filename: string): string {
  const trimmed = filename.trim();
  const dotIndex = trimmed.lastIndexOf('.');
  if (dotIndex <= 0) return trimmed;
  return trimmed.slice(0, dotIndex);
}

function formatTimestamp(value?: string | null): string {
  if (!value) return 'Never';
  return new Date(value).toLocaleString();
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function ingestionBadge(status: ProjectDocument['ingestion_status']): string {
  switch (status) {
    case 'indexed':
      return 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400';
    case 'indexing':
      return 'bg-blue-500/10 text-blue-600 dark:text-blue-400';
    case 'failed':
      return 'bg-red-500/10 text-red-600 dark:text-red-400';
    default:
      return 'bg-amber-500/10 text-amber-600 dark:text-amber-400';
  }
}

export function DocumentsTab({ projectId }: DocumentsTabProps) {
  const [documents, setDocuments] = useState<ProjectDocument[]>([]);
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);
  const [draft, setDraft] = useState<DocumentDraft>(createEmptyDraft);
  const [loadingList, setLoadingList] = useState(true);
  const [loadingContent, setLoadingContent] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingDeleteDocument, setPendingDeleteDocument] = useState<{
    id: string;
    title: string;
  } | null>(null);
  const importInputRef = useRef<HTMLInputElement | null>(null);

  const selectedDocument = useMemo(
    () => documents.find((document) => document.id === selectedDocumentId) || null,
    [documents, selectedDocumentId]
  );

  const loadDocuments = async (preferredDocumentId?: string | null) => {
    setLoadingList(true);
    setError(null);

    try {
      const list = await getProjectDocuments(projectId);
      setDocuments(list);

      const nextSelectedId =
        preferredDocumentId && list.some((document) => document.id === preferredDocumentId)
          ? preferredDocumentId
          : selectedDocumentId && list.some((document) => document.id === selectedDocumentId)
            ? selectedDocumentId
            : list[0]?.id || null;

      setSelectedDocumentId(nextSelectedId);
      if (!nextSelectedId && list.length === 0) {
        setDraft(createEmptyDraft());
      }
    } catch (loadError) {
      logger.error('Failed to load project documents:', loadError);
      setError(loadError instanceof Error ? loadError.message : 'Failed to load documents');
    } finally {
      setLoadingList(false);
    }
  };

  useEffect(() => {
    void loadDocuments(null);
  }, [projectId]);

  useEffect(() => {
    if (!selectedDocument) return;

    let cancelled = false;

    const loadDocumentContent = async () => {
      setLoadingContent(true);
      setError(null);

      try {
        const { download_url } = await getProjectDocumentDownloadUrl(
          projectId,
          selectedDocument.storage_key
        );
        const response = await fetch(download_url);
        if (!response.ok) {
          throw new Error(`Failed to download document content (${response.status})`);
        }
        const content = await response.text();
        if (cancelled) return;

        setDraft({
          documentId: selectedDocument.id,
          title: selectedDocument.title,
          filename: selectedDocument.filename,
          documentKind: selectedDocument.document_kind,
          contentType: selectedDocument.content_type,
          content,
        });
      } catch (loadError) {
        logger.error('Failed to load project document content:', loadError);
        if (!cancelled) {
          setError(
            loadError instanceof Error
              ? loadError.message
              : 'Failed to load selected document content'
          );
        }
      } finally {
        if (!cancelled) {
          setLoadingContent(false);
        }
      }
    };

    void loadDocumentContent();

    return () => {
      cancelled = true;
    };
  }, [projectId, selectedDocument]);

  const handleNewDocument = () => {
    setSelectedDocumentId(null);
    setDraft(createEmptyDraft());
    setError(null);
  };

  const handleImportFile = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file) return;

    try {
      const content = await file.text();
      const contentType = guessContentType(file.name, file.type);
      setSelectedDocumentId(null);
      setDraft({
        documentId: null,
        title: stripFilenameExtension(file.name),
        filename: file.name,
        documentKind: 'notes',
        contentType,
        content,
      });
      setError(null);
    } catch (readError) {
      logger.error('Failed to read imported document file:', readError);
      setError(readError instanceof Error ? readError.message : 'Failed to import file');
    }
  };

  const handleSave = async () => {
    if (saving) return;

    const trimmedTitle = draft.title.trim();
    const trimmedFilename = draft.filename.trim();
    if (!trimmedTitle || !trimmedFilename) {
      setError('Title and filename are required.');
      return;
    }

    setSaving(true);
    setError(null);

    try {
      const blob = new Blob([draft.content], { type: draft.contentType });
      const { upload_url, key } = await getProjectDocumentUploadUrl(projectId, {
        filename: trimmedFilename,
        content_type: draft.contentType,
      });

      const uploadResponse = await fetch(upload_url, {
        method: 'PUT',
        headers: {
          'Content-Type': draft.contentType,
        },
        body: blob,
      });

      if (!uploadResponse.ok) {
        throw new Error(`Upload failed with status ${uploadResponse.status}`);
      }

      const savedDocument = draft.documentId
        ? await updateProjectDocument(projectId, draft.documentId, {
            title: trimmedTitle,
            document_kind: draft.documentKind,
            content_type: draft.contentType,
            storage_key: key,
            checksum: null,
            size_bytes: blob.size,
          })
        : await createProjectDocument(projectId, {
            title: trimmedTitle,
            filename: trimmedFilename,
            document_kind: draft.documentKind,
            content_type: draft.contentType,
            storage_key: key,
            checksum: null,
            size_bytes: blob.size,
            source: 'upload',
          });

      await loadDocuments(savedDocument.id);
      setDraft((prev) => ({
        ...prev,
        documentId: savedDocument.id,
        title: savedDocument.title,
        filename: savedDocument.filename,
      }));
    } catch (saveError) {
      logger.error('Failed to save project document:', saveError);
      setError(saveError instanceof Error ? saveError.message : 'Failed to save document');
    } finally {
      setSaving(false);
    }
  };

  const handleRequestDelete = () => {
    if (!draft.documentId || deleting) return;

    setPendingDeleteDocument({
      id: draft.documentId,
      title: draft.title.trim() || draft.filename.trim() || 'this document',
    });
  };

  const handleCloseDeleteModal = () => {
    if (deleting) return;
    setPendingDeleteDocument(null);
  };

  const handleConfirmDelete = async () => {
    const documentToDelete = pendingDeleteDocument;
    if (!documentToDelete || deleting) return;

    setDeleting(true);
    setError(null);

    try {
      await deleteProjectDocument(projectId, documentToDelete.id);
      const deletedId = documentToDelete.id;
      setDraft(createEmptyDraft());
      setSelectedDocumentId(null);
      setPendingDeleteDocument(null);
      await loadDocuments(
        selectedDocumentId === deletedId ? null : selectedDocumentId
      );
    } catch (deleteError) {
      logger.error('Failed to delete project document:', deleteError);
      setError(deleteError instanceof Error ? deleteError.message : 'Failed to delete document');
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="grid gap-4 xl:grid-cols-[320px_minmax(0,1fr)]">
      <div className="bg-card border border-border rounded-xl overflow-hidden">
        <div className="px-4 py-3 border-b border-border bg-muted/50 flex items-center justify-between gap-2">
          <div>
            <h3 className="text-sm font-bold text-card-foreground">Documents</h3>
            <p className="text-xs text-muted-foreground mt-1">
              Project-specific knowledge stored in the vault.
            </p>
          </div>
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              onClick={() => void loadDocuments(selectedDocumentId)}
              className="px-2.5 py-1.5 text-xs font-medium rounded-md border border-border text-muted-foreground hover:text-card-foreground hover:bg-card transition-colors"
            >
              Refresh
            </button>
            <button
              type="button"
              onClick={handleNewDocument}
              className="px-2.5 py-1.5 text-xs font-bold rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              New
            </button>
          </div>
        </div>

        <div className="divide-y divide-border max-h-[720px] overflow-y-auto">
          {loadingList ? (
            <div className="p-4 text-sm text-muted-foreground">Loading documents...</div>
          ) : documents.length === 0 ? (
            <div className="p-6 text-center">
              <span className="material-symbols-outlined text-muted-foreground/50 text-3xl mb-2">
                library_books
              </span>
              <p className="text-sm text-muted-foreground">No documents yet</p>
              <p className="text-xs text-muted-foreground/70 mt-1">
                Create a project vault document to give the agent durable project knowledge.
              </p>
            </div>
          ) : (
            documents.map((document) => (
              <button
                key={document.id}
                type="button"
                onClick={() => setSelectedDocumentId(document.id)}
                className={`w-full text-left px-4 py-3 transition-colors ${
                  selectedDocumentId === document.id
                    ? 'bg-primary/10'
                    : 'hover:bg-muted/50'
                }`}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="text-sm font-semibold text-card-foreground truncate">
                      {document.title}
                    </p>
                    <p className="text-xs text-muted-foreground truncate mt-1">
                      {document.filename}
                    </p>
                  </div>
                  <span
                    className={`text-[10px] font-bold px-2 py-0.5 rounded-full ${ingestionBadge(
                      document.ingestion_status
                    )}`}
                  >
                    {document.ingestion_status}
                  </span>
                </div>
                <div className="mt-2 flex items-center justify-between gap-3 text-[11px] text-muted-foreground">
                  <span>{DOCUMENT_KIND_LABELS[document.document_kind]}</span>
                  <span>v{document.version}</span>
                </div>
              </button>
            ))
          )}
        </div>
      </div>

      <div className="bg-card border border-border rounded-xl overflow-hidden">
        <div className="px-4 py-3 border-b border-border bg-muted/50 flex flex-wrap items-center justify-between gap-2">
          <div>
            <h3 className="text-sm font-bold text-card-foreground">
              {draft.documentId ? 'Edit Document' : 'New Document'}
            </h3>
            <p className="text-xs text-muted-foreground mt-1">
              Upload markdown or structured text so it can be indexed in later phases.
            </p>
          </div>
          <div className="flex items-center gap-1.5">
            <input
              ref={importInputRef}
              type="file"
              accept={IMPORT_FILE_ACCEPT}
              className="hidden"
              onChange={(event) => {
                void handleImportFile(event);
              }}
            />
            <button
              type="button"
              onClick={() => importInputRef.current?.click()}
              className="px-2.5 py-1.5 text-xs font-medium rounded-md border border-border text-muted-foreground hover:text-card-foreground hover:bg-card transition-colors"
            >
              Import file
            </button>
            {draft.documentId && (
              <button
                type="button"
                onClick={() => {
                  handleRequestDelete();
                }}
                className="px-2.5 py-1.5 text-xs font-medium rounded-md border border-red-500/40 text-red-500 hover:bg-red-500/10 transition-colors"
                disabled={deleting}
              >
                {deleting ? 'Deleting...' : 'Delete'}
              </button>
            )}
            <button
              type="button"
              onClick={() => {
                void handleSave();
              }}
              className="px-3 py-1.5 text-xs font-bold rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              disabled={saving || loadingContent}
            >
              {saving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>

        <div className="p-4 space-y-4">
          {error && (
            <div className="rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-400">
              {error}
            </div>
          )}

          {loadingContent && (
            <div className="rounded-lg border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
              Loading document content...
            </div>
          )}

          <div className="grid gap-4 md:grid-cols-2">
            <div>
              <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">
                Title
              </label>
              <input
                type="text"
                value={draft.title}
                onChange={(event) =>
                  setDraft((prev) => ({ ...prev, title: event.target.value }))
                }
                className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                placeholder="Authentication API"
              />
            </div>
            <div>
              <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">
                Filename
              </label>
              <input
                type="text"
                value={draft.filename}
                onChange={(event) => {
                  const nextFilename = event.target.value;
                  setDraft((prev) => ({
                    ...prev,
                    filename: nextFilename,
                    contentType: guessContentType(nextFilename, prev.contentType),
                  }));
                }}
                disabled={Boolean(draft.documentId)}
                className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary disabled:opacity-60"
                placeholder="auth-api.md"
              />
              {draft.documentId && (
                <p className="text-[11px] text-muted-foreground mt-1">
                  Filename stays fixed in v1 because it is the document upsert key.
                </p>
              )}
            </div>
            <div>
              <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">
                Document kind
              </label>
              <select
                value={draft.documentKind}
                onChange={(event) =>
                  setDraft((prev) => ({
                    ...prev,
                    documentKind: event.target.value as ProjectDocumentKind,
                  }))
                }
                className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
              >
                {Object.entries(DOCUMENT_KIND_LABELS).map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">
                Content type
              </label>
              <input
                list="project-document-content-types"
                value={draft.contentType}
                onChange={(event) =>
                  setDraft((prev) => ({ ...prev, contentType: event.target.value }))
                }
                className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary"
                placeholder="text/markdown"
                spellCheck={false}
              />
              <datalist id="project-document-content-types">
                {CONTENT_TYPE_SUGGESTIONS.map(({ value, label }) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </datalist>
              <p className="text-[11px] text-muted-foreground mt-1">
                Suggested MIME types are listed, but you can enter any custom text-based content type.
              </p>
            </div>
          </div>

          {selectedDocument && (
            <div className="rounded-lg border border-border bg-muted/40 px-3 py-2 grid gap-2 md:grid-cols-4 text-xs text-muted-foreground">
              <div>
                <span className="font-semibold text-card-foreground">Version</span>
                <p className="mt-1">v{selectedDocument.version}</p>
              </div>
              <div>
                <span className="font-semibold text-card-foreground">Status</span>
                <p className="mt-1 capitalize">{selectedDocument.ingestion_status}</p>
              </div>
              <div>
                <span className="font-semibold text-card-foreground">Updated</span>
                <p className="mt-1">{formatTimestamp(selectedDocument.updated_at)}</p>
              </div>
              <div>
                <span className="font-semibold text-card-foreground">Size</span>
                <p className="mt-1">{formatBytes(selectedDocument.size_bytes)}</p>
              </div>
              {selectedDocument.index_error && (
                <div className="md:col-span-4">
                  <span className="font-semibold text-red-500">Index error</span>
                  <p className="mt-1 text-red-400">{selectedDocument.index_error}</p>
                </div>
              )}
            </div>
          )}

          <div>
            <label className="block text-xs font-bold text-muted-foreground uppercase mb-1.5">
              Content
            </label>
            <textarea
              value={draft.content}
              onChange={(event) =>
                setDraft((prev) => ({ ...prev, content: event.target.value }))
              }
              rows={22}
              className="w-full bg-muted border border-border rounded-lg px-3 py-3 text-sm text-card-foreground focus:ring-primary focus:border-primary font-mono resize-y"
              placeholder="# Authentication API&#10;&#10;Document the endpoints, payloads, and constraints here."
            />
          </div>
        </div>
      </div>

      <ConfirmModal
        isOpen={!!pendingDeleteDocument}
        onClose={handleCloseDeleteModal}
        onConfirm={handleConfirmDelete}
        title="Delete Document"
        message={`Delete "${pendingDeleteDocument?.title ?? ''}"? This action cannot be undone.`}
        confirmText="Delete Document"
        confirmVariant="danger"
        isLoading={deleting}
      />
    </div>
  );
}
