# Implementation Plan: Project Document Vault & Task Context

This document outlines the breakdown of the Project Document Vault feature, the system impact analysis, and a step-by-step implementation checklist.

## 1. Feature Breakdown & Impact Analysis

The feature is divided into three main logical components. Here is the analysis of each component's impact on the existing codebase:

### Phase 1: Task Context (Direct Prompt Injection)
**Description:** Allow attaching specific context (text, markdown, images) directly to a Task. This context is injected directly into the Agent's prompt when handling that specific Task.
**Impact Level:** Low - Medium.
**Affected Areas:**
- **Database (Rust `sea-orm`)**: 
  - Need to update the `tasks` table schema. Either add a `context_files` JSONB column OR create a new `task_attachments` table with a foreign key to `tasks`.
- **Workflow & UI (Next.js)**:
  - **Task Modal**: Needs to be updated to support uploading/pasting text and image URLs.
  - **Kanban Board**: May need an indicator (e.g., a paperclip icon) to show if a Task has attached context.
  - **Task Execution Flow**: The presence of `context_files` changes how the Agent behaves for *this specific Task*. Instead of a generic code edit, the Agent must prioritize the provided context (e.g., matching the provided UI mockup or error stack trace).
- **Agent Core (`crates/core`)**:
  - Update `spawner.rs` or the prompt generation logic. When picking up a Task from the "Todo" or "In Progress" column, the system must retrieve the `context_files` and prepend/append it as a highly-prioritized `SystemMessage` or initial `UserMessage` to guide the Agent's first action.
  - **Prompt Injection Detail:** The prompt needs a clear boundary. Example: 
    ```
    You are working on the task: {task.title}. 
    === TASK CONTEXT ===
    {task.context_files_content}
    ====================
    Please strictly adhere to the constraints and designs provided in the TASK CONTEXT above.
    ```

### Phase 2: Project Document Vault (Core Entity & Storage)
**Description:** Create the CRUD operations and storage mechanism for project-level documents.
**Impact Level:** Medium.
**Affected Areas:**
- **Database (Rust)**:
  - Create a new `project_documents` table (Columns: `id`, `project_id`, `title`, `content_type`, `raw_content`, `created_at`).
  - Update `projects` entity module to include the relation to documents.
- **API Engine (`crates/engine`)**:
  - Create REST/GraphQL endpoints for CRUD operations on Project Documents.
- **Frontend (Next.js)**:
  - Replace the existing "Deployment" tab inside the Project Dashboard layout with the new "Documents" tab (temporarily disabling Deployment features).
  - Implement a Document Viewer/Editor (Markdown editor).

### Phase 3: RAG Integration & Agent Tools
**Description:** Implement text chunking, vector embedding generation, and the AI Tool for the Agent to search the Vault.
**Impact Level:** High. (Depends on the progress of the Global Knowledge Base feature `sqlite-vec` integration).
**Affected Areas:**
- **Database (Rust)**:
  - Create `project_document_chunks` table using `sqlite-vec` extension for storing `f32` vectors.
- **Background Worker (`crates/workers` or async tasks)**:
  - Implement a pipeline: When a document is saved/updated -> Split into chunks -> Call `fastembed` -> Save to `project_document_chunks`.
- **Agent Core (`crates/core/src/tools`)**:
  - Implement a new `Tool`: `search_project_vault_tool(query: String)`.
  - The tool will compute the embedding of the `query` -> run Cosine Similarity against `project_document_chunks` where `project_id = current_project` -> Return Top-K chunks to the Agent.

### Phase 4: OpenClaw Gateway APIs Integration
**Description:** Expose the Vault and Task Context functionalities to the OpenClaw Super Admin control plane via mirrored APIs, allowing external systems to manage project knowledge.
**Impact Level:** Medium.
**Affected Areas:**
- **`crates/engine` (API Layer)**:
  - **Vault APIs:**
    - `POST /openclaw/api/v1/projects/{project_id}/documents`: Ingest new documents from external sources (e.g., syncing from a corporate wiki like Confluence).
    - `GET /openclaw/api/v1/projects/{project_id}/documents/{id}`: Retrieve document details.
    - `DELETE /openclaw/api/v1/projects/{project_id}/documents/{id}`: Remove stale documents.
  - **Task Context APIs:**
    - `POST /openclaw/api/v1/tasks/{task_id}/context`: Attach context to a task from an external trigger (e.g., an automated Jira webhook that creates a task in ACPMS and pushes the Jira description as context).
- **Authentication/Security**: Ensure these endpoints strictly validate the OpenClaw `JWT_SECRET` and authorization headers.

---

## 2. Implementation Checklist

Below is the step-by-step checklist to implement this feature. 

### Phase 1: Task Context (Quick Win)
- [ ] **DB Migration**: Add `context_data` (JSON / Text) field to the `tasks` table.
- [ ] **API Update**: Update the Task CRUD endpoints to accept and return the new context field.
- [ ] **Frontend UI**: Add a Markdown editor/Upload area in the Task Detail Modal.
- [ ] **Agent Spawner**: Update `crates/core/src/agents/spawner.rs` to read `context_data` and construct the high-priority System Prompt boundary.
- [ ] **Verification**: Create a task with specific context (e.g., "Use purple for the button"), run the agent, and verify the agent's code uses purple.

### Phase 2: Project Vault CRUD
- [ ] **DB Migration**: Create `project_documents` table.
- [ ] **Rust Models**: Generate `sea-orm` entities for the new table. Ensure it has `created_at` and `updated_at` fields.
- [ ] **Rust API**: Implement internal endpoints (`GET`, `POST`, `PUT`, `DELETE`) for `/projects/{id}/documents`. **Crucial:** Implement "Upsert" logic. If a document with the same `title` or `filename` is uploaded, overwrite the existing one and update `updated_at` to ensure only the latest version exists.
- [ ] **Frontend UI**: Replace the "Deployment" tab with the new `/projects/[id]/documents` page.
- [ ] **Frontend UI**: Implement document listing, creating, and editing interfaces.
- [ ] **Verification**: Create a test project, add a markdown document, edit it, and delete it via the UI. Ensure data persists correctly in the DB.

### Phase 3: Vector RAG & Tool Integration
- [ ] **Dependency Check**: Ensure `sqlite-vec` and `fastembed` are properly set up (overlap with Global KB).
- [ ] **DB Migration**: Create `project_document_chunks` virtual/vector table.
- [ ] **Chunking Engine**: Implement a Rust text-splitter to process markdown text.
- [ ] **Embedding Pipeline**: Trigger the embedding generation when a `project_document` is saved. **Crucial:** If updating an existing document, the pipeline must DELETE the old chunks from `project_document_chunks` before inserting the new ones to avoid stale context.
- [ ] **Agent Tool**: Create `SearchProjectVault` struct implementing the Tool trait.
- [ ] **Tool Registration**: Inject this tool into the Agent's tool registry when the Session belongs to a project.
- [ ] **Verification**: Upload a document containing a unique secret code (e.g., "The secret code is 998877"). Run the agent on a task asking "What is the secret code?". The agent should use the tool, find the document, and answer correctly.

### Phase 4: OpenClaw Gateway Endpoints
- [ ] **API Controller**: Create `projects_openclaw_handlers.rs` to expose the new Vault endpoints under `/openclaw/api/v1/`.
- [ ] **API Controller**: Update the Task OpenClaw handlers to accept task context uploads (`POST /openclaw/api/v1/tasks/{task_id}/context`).
- [ ] **Security Validation**: Apply the existing OpenClaw auth middleware to these new routes.
- [ ] **Verification**: Use Postman/curl with a valid JWT to successfully post a document to a project and context to a task.
