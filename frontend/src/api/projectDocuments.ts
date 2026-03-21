import { apiDelete, apiGet, apiPatch, apiPost, API_PREFIX } from './client';

export type ProjectDocumentKind =
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

export type ProjectDocumentSource = 'upload' | 'repo_sync' | 'api';
export type ProjectDocumentIngestionStatus = 'pending' | 'indexing' | 'indexed' | 'failed';

export interface ProjectDocument {
  id: string;
  project_id: string;
  title: string;
  filename: string;
  document_kind: ProjectDocumentKind;
  content_type: string;
  storage_key: string;
  checksum?: string | null;
  size_bytes: number;
  source: ProjectDocumentSource;
  version: number;
  ingestion_status: ProjectDocumentIngestionStatus;
  index_error?: string | null;
  indexed_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateProjectDocumentRequest {
  title: string;
  filename: string;
  document_kind: ProjectDocumentKind;
  content_type: string;
  storage_key: string;
  checksum?: string | null;
  size_bytes: number;
  source: ProjectDocumentSource;
}

export interface UpdateProjectDocumentRequest {
  title?: string;
  document_kind?: ProjectDocumentKind;
  content_type?: string;
  storage_key?: string;
  checksum?: string | null;
  size_bytes?: number;
}

export interface ProjectDocumentUploadUrlRequest {
  filename: string;
  content_type: string;
}

export interface ProjectDocumentUploadUrlResponse {
  upload_url: string;
  key: string;
}

export interface ProjectDocumentDownloadUrlResponse {
  download_url: string;
}

export async function getProjectDocuments(projectId: string): Promise<ProjectDocument[]> {
  return apiGet<ProjectDocument[]>(`${API_PREFIX}/projects/${projectId}/documents`);
}

export async function getProjectDocument(
  projectId: string,
  documentId: string
): Promise<ProjectDocument> {
  return apiGet<ProjectDocument>(`${API_PREFIX}/projects/${projectId}/documents/${documentId}`);
}

export async function getProjectDocumentUploadUrl(
  projectId: string,
  data: ProjectDocumentUploadUrlRequest
): Promise<ProjectDocumentUploadUrlResponse> {
  return apiPost<ProjectDocumentUploadUrlResponse>(
    `${API_PREFIX}/projects/${projectId}/documents/upload-url`,
    data
  );
}

export async function createProjectDocument(
  projectId: string,
  data: CreateProjectDocumentRequest
): Promise<ProjectDocument> {
  return apiPost<ProjectDocument>(`${API_PREFIX}/projects/${projectId}/documents`, data);
}

export async function updateProjectDocument(
  projectId: string,
  documentId: string,
  data: UpdateProjectDocumentRequest
): Promise<ProjectDocument> {
  return apiPatch<ProjectDocument>(
    `${API_PREFIX}/projects/${projectId}/documents/${documentId}`,
    data
  );
}

export async function deleteProjectDocument(
  projectId: string,
  documentId: string
): Promise<void> {
  return apiDelete(`${API_PREFIX}/projects/${projectId}/documents/${documentId}`);
}

export async function getProjectDocumentDownloadUrl(
  projectId: string,
  key: string
): Promise<ProjectDocumentDownloadUrlResponse> {
  return apiPost<ProjectDocumentDownloadUrlResponse>(
    `${API_PREFIX}/projects/${projectId}/documents/download-url`,
    { key }
  );
}
