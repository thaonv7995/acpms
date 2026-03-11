# Implementation Plan: Project Document Vault & Task Context

This document outlines the breakdown of the Project Document Vault feature, the system impact analysis, and a step-by-step implementation checklist.

Normative schema and API details are defined in [02_data_model_and_api_contracts.md](/Users/thaonv/Projects/Personal/Agentic-Coding/docs/plan-feature/project-document-vault/02_data_model_and_api_contracts.md). Implementers should read that file before touching migrations or routes.

## Current Status

- Phase 1 implementation landed in commit `66fe5ac`.
- Phase 2 implementation landed in commit `9a6cb67`.
- Phase 3 implementation landed in commit `5db499d`.
- Phase 4 implementation landed in commit `6bd6525`.
- Manual end-to-end UI/curl verification items below are still pending unless explicitly checked.

## 1. Feature Breakdown & Impact Analysis

The feature is divided into three main logical components. Here is the analysis of each component's impact on the existing codebase:

### Phase 1: Task Context (Direct Prompt Injection)
**Description:** Allow attaching specific context (text, markdown, images) directly to a Task. This context is injected directly into the Agent's prompt when handling that specific Task.
**Impact Level:** Low - Medium.
**Affected Areas:**
- **Database (Rust `sea-orm`)**: 
  - Create a new `task_contexts` table (id, task_id, title, content_type, raw_content, source, sort_order, created_by, updated_by, created_at, updated_at). DO NOT overload the `metadata` JSONB field.
  - Create a new `task_context_attachments` table to map S3/Storage files to a task context. Migrate existing attachment logic if necessary to avoid conflicting sources of truth.
  - **Task Execution Flow**: The presence of contextual data changes how the Agent behaves for *this specific Task*. Instead of a generic code edit, the Agent must prioritize the provided context (e.g., matching the provided UI mockup or error stack trace).
  - **Prompt Injection Detail:** The prompt needs a clear boundary. Example: 
    ```
    You are working on the task: {task.title}. 
    === TASK CONTEXT ===
    {task.context_content}
    ====================
    Please strictly adhere to the constraints and designs provided in the TASK CONTEXT above.
    ```
- **API Engine (`crates/server`)**:
  - Add internal API endpoints for `task_contexts`.
  - Add presigned URL generation endpoints for `task_context_attachments` (e.g., `POST /api/v1/tasks/{id}/context-attachments/upload-url`), mimicking `tasks::get_task_attachment_upload_url`. The generated key should follow the pattern `task-context-attachments/{project_id}/{task_id}/{uuid}-{filename}`.
  - Add attachment metadata endpoints plus a download-url endpoint so the UI can preview or link unsupported files.
  - **Instruction Builder**: Update `crates/server/src/routes/task_attempts.rs` to fetch context and implement a **handoff workflow** *before* the `AgentJob` is created. This service will use `storage_service.get_log_bytes(&key)` to download text/markdown payloads, extract the text, and inject it securely into the instruction string passed to the executor. For large/unsupported files, insert the presigned download URL instead.
- **Frontend (React)**:
  - Update `CreateTaskModal`/`EditTaskModal` to use the new `task_contexts` API instead of dumping into metadata. For attachments, use the new `upload-url` endpoint to push files to S3/Storage, then save the `key` to `task_context_attachments`.
  - Update `TaskDetailPage` to display the "Task Context" (list of attachments, status, content preview). Show warnings if the context exceeds prompt limits.
- **Agent Core (`acpms_executors`)**:
  - The `AgentJob` in `crates/executors/src/job_queue.rs` remains unchanged structurally; it will simply receive the pre-resolved instruction string containing the context block from the server layer.

### Phase 2: Project Document Vault (Core Entity & Storage)
**Description:** Create the CRUD operations and storage mechanism for project-level documents. Note: The Vault acts as the source of truth for raw documentation files (architecture designs, API specs, business logic docs). This complements, but does not replace, the `requirements` table or `architecture_config`. The Vault stores the raw text/markdown, whereas `requirements` represent structured goals parsed from these documents.
**Impact Level:** Medium.
**Affected Areas:**
- **Database (Rust)**:
  - Create a `project_documents` table to store metadata. Required columns: `id`, `project_id`, `title`, `filename`, `document_kind`, `content_type`, `storage_key` (for object storage, do not store huge raw content in DB), `checksum`, `size_bytes`, `source` (upload, repo_sync, api), `version`, `ingestion_status`, `index_error`, `indexed_at`, `created_by`, `updated_by`, `created_at`, `updated_at`.
  - Update `projects` entity module to include the relation to documents.
- **API Engine (`crates/server`)**:
  - Create REST endpoints for CRUD operations on Project Documents.
  - Add presigned upload-url and download-url endpoints because project documents are storage-backed in v1.
- **Frontend (React)**:
  - Add a new "Documents" tab inside the Project Dashboard layout (Do NOT replace the existing "Deployments" tab).
  - Implement a Document Viewer/Editor (Markdown editor).

### Phase 3: RAG Integration & Agent Tools
**Description:** Implement text chunking, vector embedding generation, and the AI Tool for the Agent to search the Vault.
**Impact Level:** High. (Depends on the progress of the Global Knowledge Base feature `sqlite-vec` integration).
**Affected Areas:**
- **Database (Rust)**:
  - Create `project_document_chunks` table using `sqlite-vec` extension for storing `f32` vectors.
- **Background Worker (`acpms_executors` or async tasks)**:
  - Implement a pipeline: When a document is saved/updated -> Split into chunks -> Call `fastembed` -> Save to `project_document_chunks`.
- **Executor / Agent Integration (`acpms_executors` + server wiring)**:
  - Expose a Project Vault search capability through the existing executor/runtime integration path used for agent-side tool calls. Do not assume a shared `Tool` trait or `src/tools` module already exists in this repo.
  - The search capability should compute the embedding of the `query` -> run Cosine Similarity against `project_document_chunks` where `project_id = current_project` -> Return Top-K chunks to the Agent.

### Phase 4: OpenClaw Gateway APIs Integration
**Description:** Expose the Vault and Task Context functionalities to the OpenClaw Super Admin control plane via mirrored APIs, allowing external systems to manage project knowledge.
**Impact Level:** Medium.
**Affected Areas:**
- **`crates/server/src/routes/openclaw.rs` (API Layer)**:
  - **Vault APIs:**
    - `POST /api/openclaw/v1/projects/{project_id}/documents`: Ingest new documents from external sources.
    - `GET /api/openclaw/v1/projects/{project_id}/documents/{id}`: Retrieve document details.
    - `DELETE /api/openclaw/v1/projects/{project_id}/documents/{id}`: Remove stale documents.
  - **Task Context APIs:**
    - `POST /api/openclaw/v1/tasks/{task_id}/context`: Attach context to a task from an external trigger.
- **Authentication/Security**: Ensure these endpoints strictly validate the `OPENCLAW_API_KEY` using the existing auth middleware.

---

## 2. Implementation Checklist

Below is the step-by-step checklist to implement this feature. 

### Phase 1: Task Context (Quick Win)
- [x] **DB Migration**: Create `task_contexts` and `task_context_attachments` tables.
- [x] **API Update**: Implement CRUD endpoints for `task_contexts`, attachment metadata endpoints, upload URL generation (`/api/v1/tasks/{id}/context-attachments/upload-url`), and attachment download URL generation.
- [x] **Frontend UI**: Update `TaskDetailPage` to show the context, attachments, and their resolved status before the user hits "Start Agent".
- [x] **API Controller**: Update instruction builder in `crates/server/src/routes/task_attempts.rs` to pull context and attachment keys from the new tables.
- [x] **API Controller**: Implement file hand-off mechanism in `task_attempts.rs` (before job creation): fetch bytes via `storage_service` using the attachment `key`, apply MIME whitelist max-size limits (similar to `project_assistant.rs::resolve_attachments`), extract text, and inject it into the `instruction` string passed to the `AgentJob`.
- [ ] **Verification**: Create a task with specific context (e.g., "Use purple for the button"), run the agent, and verify the agent's code uses purple.

### Phase 2: Project Vault CRUD
- [x] **DB Migration**: Create `project_documents` table.
- [x] **DB Migration**: Create logical `project_document_chunks` storage required by sqlite-vec integration.
- [x] **Rust Models**: Generate `sea-orm` entities for the new table. Ensure it has `created_at` and `updated_at` fields.
- [x] **Rust API**: Implement internal endpoints for listing, creating, updating, deleting, upload-url, and download-url on `/projects/{id}/documents`. **Crucial:** Upsert by `filename`; reject ambiguous title collisions across different filenames with `409`.
- [x] **Frontend UI**: Add the new "Documents" tab inside the existing `/projects/:id` `ProjectDetailPage` flow. If nested routing is introduced later, keep `/projects/:id` as the current stable entry point and do not replace the Deployments tab.
- [x] **Frontend UI**: Implement document listing, creating, and editing interfaces.
- [ ] **Verification**: Create a test project, add a markdown document, edit it, and delete it via the UI. Ensure data persists correctly in the DB.

### Phase 3: Vector RAG & Tool Integration
Current implementation note: chunking, indexing, and runtime search are shipped, but they currently use the in-repo deterministic embedding path rather than a shared `sqlite-vec` + `fastembed` stack.

- [ ] **Dependency Check**: Ensure `sqlite-vec` and `fastembed` are properly set up (overlap with Global KB).
- [x] **Chunking Engine**: Implement a Rust text-splitter to process markdown text.
- [x] **Embedding Pipeline**: Trigger the embedding generation when a `project_document` is saved. **Crucial:** Chunk replacement must be atomic from the reader's point of view. Use a DB transaction or a staging-and-swap strategy so failed re-indexing never leaves the document with zero searchable chunks.
- [x] **Executor Integration**: Add a Project Vault search capability through the concrete executor/runtime path used for agent-side tool calls in this repo. If a shared abstraction is introduced, document that new abstraction explicitly rather than assuming a pre-existing `Tool` trait.
- [x] **Executor Wiring**: Ensure the capability is only available when the Session/Attempt belongs to a project and that tool-call logging remains compatible with the current orchestrator/event model.
- [ ] **Verification**: Upload a document containing a unique secret code (e.g., "The secret code is 998877"). Run the agent on a task asking "What is the secret code?". The agent should use the tool, find the document, and answer correctly.

### Phase 4: OpenClaw Gateway Endpoints
- [x] **API Controller**: Extend the existing `crates/server/src/routes/openclaw.rs` route set to expose the new Vault endpoints under `/api/openclaw/v1/`. Split helper modules only if the file becomes too large.
- [x] **API Controller**: Update the Task OpenClaw handlers in the same route layer to accept task context uploads (`POST /api/openclaw/v1/tasks/{task_id}/context`).
- [x] **Security Validation**: Apply the existing OpenClaw auth middleware to these new routes (requires `OPENCLAW_API_KEY`).
- [ ] **Verification**: Use Postman/curl with a valid `OPENCLAW_API_KEY` to successfully post a document to a project and context to a task.
