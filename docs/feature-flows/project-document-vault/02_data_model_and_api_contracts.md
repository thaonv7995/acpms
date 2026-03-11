# Project Document Vault & Task Context: 02 - Data Model & API Contracts

This document is the normative implementation contract for the `project-document-vault` feature. If this file conflicts with older wording in the concept doc, this file wins for schema, API, and processing behavior.

## 1. Scope

This contract covers:

- database schema for Task Context and Project Vault
- internal REST contracts used by the web app
- attachment and storage flows
- ingestion and retrieval behavior for RAG
- OpenClaw payloads for external control-plane access

This contract does not require changing the existing generic task attachment flow already stored in `tasks.metadata`. That legacy flow may continue to exist, but new Task Context work must use the dedicated tables below.

## 2. Global Conventions

- All new business endpoints return `ApiResponse<T>` following the existing server pattern.
- All new identifiers are UUIDs.
- All timestamps are UTC.
- All storage-backed file keys must use sanitized lowercase filenames.
- Large file content MUST live in object storage, not in database text columns.
- `created_by` and `updated_by` refer to the authenticated ACPMS user when available. For OpenClaw-created records, use the resolved OpenClaw actor user.

## 2.1. RBAC Contract

Unless explicitly stated otherwise, new internal routes must follow these permission rules:

| Route group | Read permission | Mutating permission |
| --- | --- | --- |
| Task Contexts | `Permission::ViewProject` | `Permission::ModifyTask` |
| Task Context attachment upload-url | n/a | `Permission::ModifyTask` |
| Task Context attachment metadata create/delete | n/a | `Permission::ModifyTask` |
| Task Context attachment download-url | `Permission::ViewProject` | n/a |
| Project Documents | `Permission::ViewProject` | `Permission::ManageProject` |
| Project Document upload-url | n/a | `Permission::ManageProject` |
| Project Document download-url | `Permission::ViewProject` | n/a |

Rules:

- Do not introduce a new permission enum value for this feature in v1.
- Task-context write operations should follow the same role envelope as task editing, not generic project management.
- Project-vault write operations should follow the same role envelope as project administration.
- OpenClaw routes continue to use OpenClaw gateway auth rather than project-member RBAC.

## 3. Enums

### 3.1. `task_context.content_type`

Allowed values:

- `text/markdown`
- `text/plain`

### 3.2. `task_context.source`

Allowed values:

- `user`
- `openclaw`
- `system`

### 3.3. `project_document.document_kind`

Allowed values:

- `architecture`
- `api_spec`
- `database_schema`
- `business_rules`
- `runbook`
- `notes`
- `other`

### 3.4. `project_document.source`

Allowed values:

- `upload`
- `repo_sync`
- `api`

### 3.5. `project_document.ingestion_status`

Allowed values:

- `pending`
- `indexing`
- `indexed`
- `failed`

## 4. Database Contract

### 4.1. `task_contexts`

Required columns:

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | UUID | PK |
| `task_id` | UUID | FK -> `tasks.id`, `ON DELETE CASCADE`, indexed |
| `title` | VARCHAR(255) | nullable |
| `content_type` | VARCHAR(64) | not null, default `text/markdown` |
| `raw_content` | TEXT | not null, default `''` |
| `source` | VARCHAR(32) | not null, default `user` |
| `sort_order` | INTEGER | not null, default `0` |
| `created_by` | UUID | nullable |
| `updated_by` | UUID | nullable |
| `created_at` | TIMESTAMPTZ | not null |
| `updated_at` | TIMESTAMPTZ | not null |

Rules:

- A task may have multiple context blocks.
- `sort_order ASC, created_at ASC` defines display order and prompt assembly order.
- `raw_content` may be empty if the context block only contains attachments.

### 4.2. `task_context_attachments`

Required columns:

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | UUID | PK |
| `task_context_id` | UUID | FK -> `task_contexts.id`, `ON DELETE CASCADE`, indexed |
| `storage_key` | VARCHAR(512) | not null |
| `filename` | VARCHAR(255) | not null |
| `content_type` | VARCHAR(255) | not null |
| `size_bytes` | BIGINT | nullable |
| `checksum` | VARCHAR(128) | nullable |
| `created_by` | UUID | nullable |
| `created_at` | TIMESTAMPTZ | not null |

Rules:

- Add a unique constraint on `(task_context_id, storage_key)`.
- Do not duplicate `task_id` here; derive it through `task_contexts`.

### 4.3. `project_documents`

Required columns:

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | UUID | PK |
| `project_id` | UUID | FK -> `projects.id`, `ON DELETE CASCADE`, indexed |
| `title` | VARCHAR(255) | not null |
| `filename` | VARCHAR(255) | not null |
| `document_kind` | VARCHAR(32) | not null, default `other` |
| `content_type` | VARCHAR(255) | not null |
| `storage_key` | VARCHAR(512) | not null |
| `checksum` | VARCHAR(128) | nullable |
| `size_bytes` | BIGINT | not null |
| `source` | VARCHAR(32) | not null |
| `version` | INTEGER | not null, default `1` |
| `ingestion_status` | VARCHAR(32) | not null, default `pending` |
| `index_error` | TEXT | nullable |
| `indexed_at` | TIMESTAMPTZ | nullable |
| `created_by` | UUID | nullable |
| `updated_by` | UUID | nullable |
| `created_at` | TIMESTAMPTZ | not null |
| `updated_at` | TIMESTAMPTZ | not null |

Rules:

- Add a unique constraint on `(project_id, filename)`.
- `filename` is the canonical upsert key.
- If a create request uses a `title` that already exists on a different `filename`, the server should reject with `409 Conflict` instead of guessing.
- Updating document content through the API must increment `version` by `1`.

### 4.4. `project_document_chunks`

This is a logical contract. Physical storage may be one sqlite-vec table or a vec table plus companion metadata table, as long as the fields below are queryable together.

Required logical fields:

| Field | Type | Constraints |
| --- | --- | --- |
| `id` | UUID or row identifier | unique |
| `document_id` | UUID | indexed |
| `project_id` | UUID | indexed |
| `chunk_index` | INTEGER | unique per `document_id` |
| `content` | TEXT | not null |
| `content_hash` | VARCHAR(128) | not null |
| `token_count` | INTEGER | nullable |
| `embedding` | `f32[]` / vec column | not null |
| `created_at` | TIMESTAMPTZ | not null |

Rules:

- The retrieval path must be able to filter by `project_id`.
- Chunk replacement must be atomic from the reader's point of view. A failed re-index must not leave the document with zero active chunks.

## 5. Limits and Processing Rules

### 5.1. Task Context Limits

- max context blocks per task: `10`
- max `raw_content` per context block: `20_000` characters
- max stored attachments per task: `10`
- max auto-resolved attachments injected into prompt: `5`
- max attachment size for automatic text extraction: `1 MiB`

### 5.2. Task Attachment MIME Rules

Auto-extract to prompt:

- `text/*`
- `application/json`
- `text/yaml`
- `application/yaml`
- `application/x-yaml`

Vision/reference only, not text-extracted:

- `image/png`
- `image/jpeg`
- `image/webp`

Allowed as link-only fallback:

- `application/pdf`
- any other uploaded type explicitly accepted by product UI

### 5.3. Project Document Limits

- max document size in v1: `5 MiB`
- supported RAG ingestion types in v1:
  - `text/markdown`
  - `text/plain`
  - `application/json`
  - `text/yaml`
  - `application/yaml`
  - `application/x-yaml`
- `application/pdf` remains `future phase` for parsing and indexing; do not auto-index PDF in v1

### 5.4. Prompt Assembly Rules

For each task attempt, the API server builds a single block:

```text
=== TASK CONTEXT ===
[Context: <title or generated label>]
<raw_content if present>

[Attachment: <filename>]
<extracted text, if extractable>

[Attachment Link]
filename: <filename>
content_type: <content_type>
reason: <too_large|unsupported_type|vision_only>
download_url: <presigned_url>
=== END TASK CONTEXT ===
```

Rules:

- process contexts in `sort_order ASC, created_at ASC`
- process attachments in `created_at ASC`
- de-duplicate repeated `storage_key`
- if no usable content exists, omit the block entirely

## 6. Internal API Contracts

### 6.1. Shared Upload URL Shape

Request:

```json
{
  "filename": "login-spec.md",
  "content_type": "text/markdown"
}
```

Response `data`:

```json
{
  "upload_url": "https://...",
  "key": "project-documents/<project_id>/<uuid>-login-spec.md"
}
```

### 6.2. Task Context DTOs

`TaskContextAttachmentDto`

```json
{
  "id": "uuid",
  "task_context_id": "uuid",
  "storage_key": "task-context-attachments/<project_id>/<task_id>/<uuid>-mockup.png",
  "filename": "mockup.png",
  "content_type": "image/png",
  "size_bytes": 123456,
  "checksum": null,
  "created_at": "2026-03-10T09:00:00Z"
}
```

`TaskContextDto`

```json
{
  "id": "uuid",
  "task_id": "uuid",
  "title": "Login screen constraints",
  "content_type": "text/markdown",
  "raw_content": "Use the attached mockup and keep button copy in English.",
  "source": "user",
  "sort_order": 0,
  "attachments": [],
  "created_at": "2026-03-10T09:00:00Z",
  "updated_at": "2026-03-10T09:00:00Z"
}
```

### 6.3. Task Context Endpoints

`GET /api/v1/tasks/{task_id}/contexts`

- returns `ApiResponse<Vec<TaskContextDto>>`
- requires `Permission::ViewProject`

`POST /api/v1/tasks/{task_id}/contexts`

Request:

```json
{
  "title": "Login screen constraints",
  "content_type": "text/markdown",
  "raw_content": "Match the attached design. Keep CTA label as Sign in.",
  "source": "user",
  "sort_order": 0
}
```

Returns created `TaskContextDto`.
- requires `Permission::ModifyTask`

`PATCH /api/v1/tasks/{task_id}/contexts/{context_id}`

- patch semantics
- allowed fields: `title`, `content_type`, `raw_content`, `sort_order`
- requires `Permission::ModifyTask`

`DELETE /api/v1/tasks/{task_id}/contexts/{context_id}`

- deletes the context and its attachments
- requires `Permission::ModifyTask`

`POST /api/v1/tasks/{task_id}/context-attachments/upload-url`

- request body follows shared upload-url shape
- generated key format:
  - `task-context-attachments/{project_id}/{task_id}/{uuid}-{safe_filename}`
- requires `Permission::ModifyTask`

`POST /api/v1/tasks/{task_id}/contexts/{context_id}/attachments`

Request:

```json
{
  "storage_key": "task-context-attachments/<project_id>/<task_id>/<uuid>-mockup.png",
  "filename": "mockup.png",
  "content_type": "image/png",
  "size_bytes": 123456,
  "checksum": null
}
```

Returns created `TaskContextAttachmentDto`.
- requires `Permission::ModifyTask`

`DELETE /api/v1/tasks/{task_id}/contexts/{context_id}/attachments/{attachment_id}`

- removes metadata row only
- object deletion from storage is optional best-effort cleanup, not a hard requirement for v1
- requires `Permission::ModifyTask`

`POST /api/v1/tasks/{task_id}/context-attachments/download-url`

Request:

```json
{
  "key": "task-context-attachments/<project_id>/<task_id>/<uuid>-mockup.png"
}
```

Response `data`:

```json
{
  "download_url": "https://..."
}
```

- requires `Permission::ViewProject`

### 6.4. Web App Task Attachment Sequence

1. Call `POST /api/v1/tasks/{task_id}/context-attachments/upload-url`
2. Upload file directly to storage with returned URL
3. Create or update the context block
4. Call `POST /api/v1/tasks/{task_id}/contexts/{context_id}/attachments`
5. Refresh `GET /api/v1/tasks/{task_id}/contexts`

### 6.5. Project Document DTO

```json
{
  "id": "uuid",
  "project_id": "uuid",
  "title": "Authentication API",
  "filename": "auth-api.md",
  "document_kind": "api_spec",
  "content_type": "text/markdown",
  "storage_key": "project-documents/<project_id>/<uuid>-auth-api.md",
  "checksum": "sha256:...",
  "size_bytes": 4096,
  "source": "upload",
  "version": 3,
  "ingestion_status": "indexed",
  "index_error": null,
  "indexed_at": "2026-03-10T09:10:00Z",
  "created_at": "2026-03-10T09:00:00Z",
  "updated_at": "2026-03-10T09:10:00Z"
}
```

### 6.6. Project Document Endpoints

`GET /api/v1/projects/{project_id}/documents`

- returns `ApiResponse<Vec<ProjectDocumentDto>>`
- requires `Permission::ViewProject`

`GET /api/v1/projects/{project_id}/documents/{document_id}`

- returns `ApiResponse<ProjectDocumentDto>`
- requires `Permission::ViewProject`

`POST /api/v1/projects/{project_id}/documents/upload-url`

- request body follows shared upload-url shape
- generated key format:
  - `project-documents/{project_id}/{uuid}-{safe_filename}`
- requires `Permission::ManageProject`

`POST /api/v1/projects/{project_id}/documents`

Request:

```json
{
  "title": "Authentication API",
  "filename": "auth-api.md",
  "document_kind": "api_spec",
  "content_type": "text/markdown",
  "storage_key": "project-documents/<project_id>/<uuid>-auth-api.md",
  "checksum": "sha256:...",
  "size_bytes": 4096,
  "source": "upload"
}
```

Semantics:

- if `filename` does not exist in project, create new document with `version = 1`
- if `filename` already exists in project, update that row in-place and increment `version`
- if another row already uses the same `title` with a different `filename`, return `409 Conflict`
- requires `Permission::ManageProject`

`PATCH /api/v1/projects/{project_id}/documents/{document_id}`

- allowed fields:
  - `title`
  - `document_kind`
  - `content_type`
  - `storage_key`
  - `checksum`
  - `size_bytes`
- replacing `storage_key` counts as new document content and must increment `version`
- requires `Permission::ManageProject`

`DELETE /api/v1/projects/{project_id}/documents/{document_id}`

- deletes document metadata
- deletes all vector chunks for that document
- storage object deletion is best-effort
- requires `Permission::ManageProject`

`POST /api/v1/projects/{project_id}/documents/download-url`

Request:

```json
{
  "key": "project-documents/<project_id>/<uuid>-auth-api.md"
}
```

Response `data`:

```json
{
  "download_url": "https://..."
}
```

- requires `Permission::ViewProject`

### 6.7. Web App Project Document Sequence

1. Call `POST /api/v1/projects/{project_id}/documents/upload-url`
2. Upload object directly to storage
3. Call `POST /api/v1/projects/{project_id}/documents` or `PATCH`
4. Server marks document `ingestion_status = pending`
5. Async ingestion job updates `indexing -> indexed|failed`

## 7. RAG / Ingestion Contract

### 7.1. Ingestion Trigger

Trigger ingestion whenever:

- a project document is created
- a project document content object changes
- a project document is deleted

### 7.2. Ingestion Steps

1. Load object bytes from `storage_key`
2. Parse to UTF-8 text according to `content_type`
3. Split text into chunks
4. Insert the replacement chunk set into a staging transaction or versioned staging area
5. Atomically swap the active chunk set for `document_id`
6. Remove superseded chunks only after the new set is active
7. Update `project_documents.ingestion_status`

Failure rules:

- If embedding generation or chunk insertion fails, keep the previously active chunk set untouched.
- Set `ingestion_status = failed` and populate `index_error`.
- Do not delete the last known-good chunk set unless the document itself is being deleted.

### 7.3. Chunking Defaults

Use these defaults unless repository-level RAG conventions already define stricter ones:

- target chunk size: `800` tokens
- overlap: `120` tokens
- fallback when tokenizer is unavailable: `4000` characters with `500` character overlap
- default retrieval `top_k`: `5`
- hard max retrieval `top_k`: `8`

### 7.4. Search Result Shape

Logical result contract:

```json
{
  "document_id": "uuid",
  "project_id": "uuid",
  "chunk_index": 0,
  "content": "Users table uses uuid primary keys...",
  "score": 0.82,
  "title": "Database schema",
  "filename": "db-schema.md",
  "document_kind": "database_schema"
}
```

## 8. OpenClaw Contract

OpenClaw must continue authenticating with `Authorization: Bearer <OPENCLAW_API_KEY>`.

### 8.1. Create or Upsert Project Document

`POST /api/openclaw/v1/projects/{project_id}/documents`

Request:

```json
{
  "title": "Architecture Decision Record",
  "filename": "adr-auth-boundary.md",
  "document_kind": "architecture",
  "content_type": "text/markdown",
  "content_text": "# ADR\n...\n",
  "source": "api"
}
```

Rules:

- `content_text` is required in OpenClaw v1
- server persists the text into storage under:
  - `project-documents/{project_id}/openclaw/{uuid}-{safe_filename}`
- same filename upserts existing row and increments `version`

### 8.2. Get Project Document

`GET /api/openclaw/v1/projects/{project_id}/documents/{document_id}`

- returns `ProjectDocumentDto`

### 8.3. Delete Project Document

`DELETE /api/openclaw/v1/projects/{project_id}/documents/{document_id}`

- deletes document metadata and chunks

### 8.4. Create Task Context

`POST /api/openclaw/v1/tasks/{task_id}/context`

Request:

```json
{
  "title": "Imported from Jira",
  "content_type": "text/markdown",
  "raw_content": "### Jira Summary\n...\n",
  "sort_order": 0
}
```

Rules:

- OpenClaw v1 supports text-based context creation only
- binary attachment ingestion through OpenClaw is out of scope for this milestone

## 9. Acceptance Gates

Implementation is considered complete only when all of the following are true:

- web UI can create, edit, list, and delete task contexts without using `tasks.metadata`
- task context attachments upload through presigned URLs and are visible on `TaskDetailPage`
- task attempt instruction includes resolved task context before `AgentJob` submission
- project documents are storage-backed and indexed asynchronously
- replacing a project document removes stale chunks
- OpenClaw can create document records and text-based task context with `OPENCLAW_API_KEY`
